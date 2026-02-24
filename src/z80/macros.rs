#[macro_export]
macro_rules! dispatch_z {
    ($z:expr, $c0:expr, $c1:expr, $c2:expr, $c3:expr, $c4:expr, $c5:expr, $c6:expr, $c7:expr) => {
        match $z {
            0 => $c0,
            1 => $c1,
            2 => $c2,
            3 => $c3,
            4 => $c4,
            5 => $c5,
            6 => $c6,
            7 => $c7,
            _ => 4,
        }
    };
}
