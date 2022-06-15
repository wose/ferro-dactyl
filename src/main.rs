#![deny(unsafe_code)]
#![no_main]
#![no_std]

use panic_halt as _;
use stm32f4xx_hal as hal;

use core::convert::Infallible;
use hal::otg_fs::{UsbBusType, USB};
use hal::{
    gpio::{gpiob::PB15, EPin, Input, NoPin, Output, PushPull},
    pac,
    prelude::*,
    serial,
    spi::{NoMiso, NoSck, Spi},
};
use keyberon::debounce::Debouncer;
use keyberon::key_code::KbHidReport;
use keyberon::layout::{Event, Layout};
use keyberon::matrix::Matrix;
use nb::block;
use rtic::app;
use smart_leds::{brightness, hsv::RGB8, SmartLedsWrite};
use usb_device::bus::UsbBusAllocator;
use usb_device::class::UsbClass as _;
use usb_device::device::UsbDeviceState;
use ws2812_spi as ws2812;

type UsbClass = keyberon::Class<'static, UsbBusType, ()>;
type UsbDevice = usb_device::device::UsbDevice<'static, UsbBusType>;

mod layout;

trait ResultExt<T> {
    fn get(self) -> T;
}
impl<T> ResultExt<T> for Result<T, Infallible> {
    fn get(self) -> T {
        match self {
            Ok(v) => v,
            Err(e) => match e {},
        }
    }
}

#[app(device = crate::hal::pac, peripherals = true, dispatchers = [EXTI15_10])]
mod app {
    use super::*;
    use crate::ResultExt;

    #[shared]
    struct Shared {
        usb_dev: UsbDevice,
        usb_class: UsbClass,
        #[lock_free]
        layout: Layout<10, 4, 4, ()>,
    }

    #[local]
    struct Local {
        matrix: Matrix<EPin<Input>, EPin<Output<PushPull>>, 5, 4>,
        debouncer: Debouncer<[[bool; 5]; 4]>,
        timer: hal::timer::CounterHz<pac::TIM3>,
        led_timer: hal::timer::CounterHz<pac::TIM5>,
        transform: fn(Event) -> Event,
        tx: serial::Tx<hal::pac::USART1>,
        rx: serial::Rx<hal::pac::USART1>,
        buf: [u8; 4],
        ws: ws2812::Ws2812<Spi<hal::pac::SPI2, (NoSck, NoMiso, PB15)>>,
    }

    #[init(local = [bus: Option<UsbBusAllocator<UsbBusType>> = None, ep_mem: [u32; 1024] = [0; 1024]])]
    fn init(c: init::Context) -> (Shared, Local, init::Monotonics) {
        let rcc = c.device.RCC.constrain();
        let clocks = rcc
            .cfgr
            .use_hse(25.MHz())
            .sysclk(84.MHz())
            .require_pll48clk()
            .freeze();
        let gpioa = c.device.GPIOA.split();
        let gpiob = c.device.GPIOB.split();
        let gpioc = c.device.GPIOC.split();

        let mosi = gpiob.pb15;

        let spi = Spi::new(
            c.device.SPI2,
            (NoPin, NoPin, mosi),
            ws2812::MODE,
            3_500_000.Hz(),
            &clocks,
        );

        let ws = ws2812::Ws2812::new(spi);

        let usb = USB {
            usb_global: c.device.OTG_FS_GLOBAL,
            usb_device: c.device.OTG_FS_DEVICE,
            usb_pwrclk: c.device.OTG_FS_PWRCLK,
            pin_dm: gpioa.pa11.into_alternate(),
            pin_dp: gpioa.pa12.into_alternate(),
            hclk: clocks.hclk(),
        };
        *c.local.bus = Some(UsbBusType::new(usb, c.local.ep_mem));

        let pc14 = &gpioc.pc14.into_floating_input();
        let is_left = pc14.is_low();
        let transform: fn(Event) -> Event = if is_left {
            |e| e
        } else {
            |e| e.transform(|i, j| (i, 9 - j))
        };

        let (pa9, pa10) = (gpioa.pa9, gpioa.pa10);
        let pins = (pa9.into_alternate(), pa10.into_alternate());

        let mut serial = serial::Serial::new(c.device.USART1, pins, 38_400.bps(), &clocks).unwrap();
        serial.listen(serial::Event::Rxne);
        let (tx, rx) = serial.split();

        let usb_bus = c.local.bus.as_ref().unwrap();
        let usb_class = keyberon::new_class(usb_bus, ());
        let usb_dev = keyberon::new_device(usb_bus);

        // left working 'barstgcdv '
        let matrix = Matrix::new(
            [
                // messed up the connections and had to reorder the pins here
                gpiob.pb4.into_pull_up_input().erase(),
                gpiob.pb3.into_pull_up_input().erase(),
                gpioa.pa15.into_pull_up_input().erase(),
                gpiob.pb6.into_pull_up_input().erase(),
                gpiob.pb5.into_pull_up_input().erase(),
            ],
            [
                gpioa.pa0.into_push_pull_output().erase(),
                gpioa.pa1.into_push_pull_output().erase(),
                gpioa.pa2.into_push_pull_output().erase(),
                gpioa.pa3.into_push_pull_output().erase(),
            ],
        );

        let mut timer = hal::timer::Timer::new(c.device.TIM3, &clocks).counter_hz();
        timer.listen(hal::timer::Event::Update);
        timer.start(1.kHz()).unwrap();

        let mut led_timer = hal::timer::Timer::new(c.device.TIM5, &clocks).counter_hz();
        led_timer.listen(hal::timer::Event::Update);
        // led_timer.start(10.Hz()).unwrap();

        (
            Shared {
                usb_dev,
                usb_class,
                layout: Layout::new(&crate::layout::LAYERS),
            },
            Local {
                matrix: matrix.get(),
                debouncer: Debouncer::new([[false; 5]; 4], [[false; 5]; 4], 5),
                timer,
                led_timer,
                transform,
                tx,
                rx,
                buf: [0; 4],
                ws,
            },
            init::Monotonics(),
        )
    }

    #[task(binds = USART1, priority = 4, local = [rx, buf])]
    fn rx(c: rx::Context) {
        if let Ok(b) = c.local.rx.read() {
            c.local.buf.rotate_left(1);
            c.local.buf[3] = b;

            if c.local.buf[3] == b'\n' {
                if let Ok(event) = de(&c.local.buf[..]) {
                    handle_event::spawn(event).unwrap();
                }
            }
        }
    }

    #[task(binds = OTG_FS, priority = 3, shared = [usb_dev, usb_class])]
    fn usb_tx(c: usb_tx::Context) {
        (c.shared.usb_dev, c.shared.usb_class).lock(|usb_dev, usb_class| {
            if usb_dev.poll(&mut [usb_class]) {
                usb_class.poll();
            }
        });
    }

    #[task(binds = OTG_FS_WKUP, priority = 3, shared = [usb_dev, usb_class])]
    fn usb_rx(c: usb_rx::Context) {
        (c.shared.usb_dev, c.shared.usb_class).lock(|usb_dev, usb_class| {
            if usb_dev.poll(&mut [usb_class]) {
                usb_class.poll();
            }
        });
    }

    #[task(priority = 2, capacity = 8, shared = [layout])]
    fn handle_event(c: handle_event::Context, event: Event) {
        c.shared.layout.event(event)
    }

    #[task(priority = 2, shared = [usb_dev, usb_class, layout])]
    fn tick_keyberon(mut c: tick_keyberon::Context) {
        let _tick = c.shared.layout.tick();
        if c.shared.usb_dev.lock(|d| d.state()) != UsbDeviceState::Configured {
            return;
        }

        let report: KbHidReport = c.shared.layout.keycodes().collect();
        if !c
            .shared
            .usb_class
            .lock(|k| k.device_mut().set_keyboard_report(report.clone()))
        {
            return;
        }
        while let Ok(0) = c.shared.usb_class.lock(|k| k.write(report.as_bytes())) {}
    }

    #[task(binds = TIM3, priority = 1, local = [matrix, debouncer, timer, transform, tx])]
    fn tick(c: tick::Context) {
        c.local.timer.wait().ok();
        for event in c
            .local
            .debouncer
            .events(c.local.matrix.get().get())
            .map(c.local.transform)
        {
            for &b in &ser(event) {
                block!(c.local.tx.write(b)).unwrap();
            }
            handle_event::spawn(event).unwrap();
        }

        tick_keyberon::spawn().unwrap();
    }

    #[task(binds = TIM5, priority = 4, local = [ws, led_timer])]
    fn leds(c: leds::Context) {
        c.local.led_timer.wait().ok();

        const NUM_LEDS: usize = 18;
        let mut data = [RGB8::default(); NUM_LEDS];
        let leds = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17];

        for j in 0..(256 * 5) {
            for chunk in leds.chunks(5) {
                for i in 0..NUM_LEDS {
                    if chunk.contains(&i) {
                        data[i] =
                            wheel((((i * 256) as u16 / NUM_LEDS as u16 + j as u16) & 255) as u8);
                    } else {
                        data[i] = RGB8::default();
                    }
                }

                c.local
                    .ws
                    .write(brightness(data.iter().cloned(), 32))
                    .unwrap();
            }
        }
    }
}

fn de(bytes: &[u8]) -> Result<Event, ()> {
    match *bytes {
        [b'P', i, j, b'\n'] => Ok(Event::Press(i, j)),
        [b'R', i, j, b'\n'] => Ok(Event::Release(i, j)),
        _ => Err(()),
    }
}
fn ser(e: Event) -> [u8; 4] {
    match e {
        Event::Press(i, j) => [b'P', i, j, b'\n'],
        Event::Release(i, j) => [b'R', i, j, b'\n'],
    }
}

fn wheel(mut wheel_pos: u8) -> RGB8 {
    wheel_pos = 255 - wheel_pos;
    if wheel_pos < 85 {
        return (255 - wheel_pos * 3, 0, wheel_pos * 3).into();
    }
    if wheel_pos < 170 {
        wheel_pos -= 85;
        return (0, wheel_pos * 3, 255 - wheel_pos * 3).into();
    }
    wheel_pos -= 170;
    (wheel_pos * 3, 255 - wheel_pos * 3, 0).into()
}
