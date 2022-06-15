use keyberon::action::{k, l, m, Action::*, HoldTapConfig};
use keyberon::key_code::KeyCode::*;

type Action = keyberon::action::Action<()>;

const CUT: Action = m(&[LShift, Delete]);
const COPY: Action = m(&[LCtrl, Insert]);
const PASTE: Action = m(&[LShift, Insert]);
const L3_ENTER: Action = HoldTap {
    timeout: 200,
    tap_hold_interval: 0,
    config: HoldTapConfig::HoldOnOtherKeyPress,
    hold: &l(3),
    tap: &k(Enter),
};
const L1_SP: Action = HoldTap {
    timeout: 200,
    tap_hold_interval: 0,
    config: HoldTapConfig::Default,
    hold: &l(1),
    tap: &k(Space),
};
const CSPACE: Action = m(&[LCtrl, Space]);

const SHIFT_ESC: Action = HoldTap {
    timeout: 200,
    tap_hold_interval: 0,
    config: HoldTapConfig::Default,
    hold: &k(LShift),
    tap: &k(Escape),
};
const CTRL_INS: Action = HoldTap {
    timeout: 200,
    tap_hold_interval: 0,
    config: HoldTapConfig::Default,
    hold: &k(LCtrl),
    tap: &k(Insert),
};

macro_rules! s {
    ($k:ident) => {
        m(&[LShift, $k])
    };
}
macro_rules! a {
    ($k:ident) => {
        m(&[RAlt, $k])
    };
}

#[rustfmt::skip]
pub static LAYERS: keyberon::layout::Layers<10, 4, 4, ()> = keyberon::layout::layout! {
    {
        [Q  W    F     P     B     J    L  U   Y ; ],
        [A  R    S     T     G     M    N  E   I O ],
        [Z  X    C     D     V     K    H  ,   . / ],
        [t  t LGui LAlt {L1_SP} LCtrl RShift (2) RAlt  t],
    }{
        [Pause ScrollLock PScreen       t     t    t    BSpace Delete  t    t],
        [LGui     LAlt   {CTRL_INS}{SHIFT_ESC}t CapsLock Left   Down   Up Right],
        [Undo    {CUT}     {COPY}    {PASTE}  t  Enter   Home  PgDown PgUp End],
        [t       t         t          n     t    t  {L3_ENTER}  t    t    t],
    }{
        [{s!(Kb1)}{s!(Kb2)}{s!(Kb3)}{s!(Kb4)}{s!(Kb5)}{s!(Kb6)}{s!(Kb7)}{s!(Kb8)}{s!(Kb9)}{s!(Kb0)}],
        [{ k(Kb1)}{ k(Kb2)}{ k(Kb3)}{ k(Kb4)}{ k(Kb5)}{ k(Kb6)}{ k(Kb7)}{ k(Kb8)}{ k(Kb9)}{ k(Kb0)}],
        [{a!(Kb1)}{a!(Kb2)}{a!(Kb3)}{a!(Kb4)}{a!(Kb5)}{a!(Kb6)}{a!(Kb7)}{a!(Kb8)}{a!(Kb9)}{a!(Kb0)}],
        [t t t {CSPACE} t t n t t t],
    }{
        [F2   F3    F4    F5  F6 F7  F8     F9  F10  F11],
        [LGui LAlt LCtrl LShift t t RShift RCtrl LAlt RGui],
        [t    t     t     t    t t   t      t    t    t],
        [t    t     t     n    t t   n      t    t    t],
    }
};
