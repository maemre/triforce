// Formatting utilities.

use super::*;
use std::fmt::{Debug, Display, Formatter, Result};

impl Debug for Region {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        self.inner.fmt(f)
    }
}

impl Display for Color {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", self.0.get())
    }
}

impl Display for Graph {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        for (i, v) in self.color.iter().enumerate() {
            write!(f, "{}", " ".repeat(i))?;
            for color in v {
                if let Some(c) = color {
                    write!(f, " {:x}", c.0)?;
                } else {
                    write!(f, " .")?;
                }
            }
            writeln!(f)?;
        }
        Ok(())
    }
}
