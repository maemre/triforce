// Macro definitions

#[macro_export]
macro_rules! debug_only {
    {$($arg:tt)*} => {
        #[cfg(debug_assertions)]
        {
            $($arg)*
        }
    };
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        {
            eprintln!($($arg)*);
        }
    };
}
