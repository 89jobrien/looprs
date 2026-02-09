use super::{NamedTool, Plugins};

/// Thin adapters for named external binaries.
///
/// These are intentionally "dumb": they only provide tool identity + execution.
/// Higher-level modules (jj/bd/kan/tools) own argument construction and parsing.
macro_rules! define_tool {
    ($name:ident, $bin:literal) => {
        pub struct $name<'a> {
            plugins: &'a Plugins,
        }

        impl<'a> $name<'a> {
            pub fn new(plugins: &'a Plugins) -> Self {
                Self { plugins }
            }

            pub fn system() -> $name<'static> {
                $name {
                    plugins: super::system(),
                }
            }
        }

        impl NamedTool for $name<'_> {
            const NAME: &'static str = $bin;

            fn plugins(&self) -> &Plugins {
                self.plugins
            }
        }
    };
}

define_tool!(Jj, "jj");
define_tool!(Bd, "bd");
define_tool!(Kan, "kan");
define_tool!(Rg, "rg");
define_tool!(Fd, "fd");
define_tool!(Git, "git");
