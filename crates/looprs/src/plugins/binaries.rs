use super::{NamedTool, Plugins};

/// Thin adapters for named external binaries.
///
/// These are intentionally "dumb": they only provide tool identity + execution.
/// Higher-level modules (doob/tools) own argument construction and parsing.
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

define_tool!(Doob, "doob");
define_tool!(Rg, "rg");
define_tool!(Fd, "fd");
define_tool!(Git, "git");

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::plugins::registry::ToolResolver;
    use crate::plugins::runner::MockRunner;
    use std::collections::HashMap;
    use std::ffi::OsString;
    use std::os::unix::process::ExitStatusExt;
    use std::path::PathBuf;
    use std::sync::Arc;

    struct StaticResolver {
        map: HashMap<String, PathBuf>,
    }

    impl ToolResolver for StaticResolver {
        fn resolve(&self, tool: &str) -> Option<PathBuf> {
            self.map.get(tool).cloned()
        }
    }

    fn plugins_with(mock: &Arc<MockRunner>) -> Plugins {
        let mut map = HashMap::new();
        map.insert("doob".to_string(), PathBuf::from("/usr/local/bin/doob"));
        map.insert("git".to_string(), PathBuf::from("/usr/bin/git"));
        Plugins::new(mock.clone(), Arc::new(StaticResolver { map }))
    }

    fn output_ok(stdout: &str) -> std::io::Result<std::process::Output> {
        Ok(std::process::Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        })
    }

    #[test]
    fn tool_names_match_binaries() {
        assert_eq!(Doob::NAME, "doob");
        assert_eq!(Rg::NAME, "rg");
        assert_eq!(Fd::NAME, "fd");
        assert_eq!(Git::NAME, "git");
    }

    #[test]
    fn named_tool_output_routes_through_plugins() {
        let mock = Arc::new(MockRunner::new());
        let plugins = plugins_with(&mock);
        mock.push_output(output_ok("todo list"));

        let out = Doob::new(&plugins)
            .output(vec![OsString::from("list")])
            .expect("queued output must be returned");

        assert_eq!(String::from_utf8_lossy(&out.stdout), "todo list");
        let calls = mock.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].program, PathBuf::from("/usr/local/bin/doob"));
        assert_eq!(calls[0].args, vec![OsString::from("list")]);
    }

    #[test]
    fn probe_success_requires_exit_zero() {
        let mock = Arc::new(MockRunner::new());
        let plugins = plugins_with(&mock);

        mock.push_output(output_ok(""));
        assert!(Git::new(&plugins).probe_success(vec![]));

        mock.push_output(Ok(std::process::Output {
            status: std::process::ExitStatus::from_raw(1),
            stdout: Vec::new(),
            stderr: b"fatal".to_vec(),
        }));
        assert!(!Git::new(&plugins).probe_success(vec![]));
    }

    #[test]
    fn output_if_available_returns_none_for_unresolved_tool() {
        let plugins = plugins_with(&Arc::new(MockRunner::new()));
        // "rg" is not in the static resolver map → resolution fails.
        assert!(Rg::new(&plugins).output_if_available(vec![]).is_none());
    }

    #[test]
    fn is_available_reflects_resolver() {
        let plugins = plugins_with(&Arc::new(MockRunner::new()));
        assert!(Doob::new(&plugins).is_available());
        assert!(!Fd::new(&plugins).is_available());
    }
}
