//! Declarative table of knowledge-source merge rules (see `docs/architecture.md` for details).

/// Kind of knowledge source. Merging currently only distinguishes Ruby-source re-application vs external origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    RubySource,
    RbsDecl,
    RbiGenerated,
    RbiHandwritten,
    Schema,
    Plugin,
}

impl SourceKind {
    pub fn is_user_source(self) -> bool {
        matches!(self, SourceKind::RubySource)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclKind {
    Method,
    Superclass,
    Mixin,
    Constant,
    IsModule,
    TypeParams,
    Ivar,
    DirtyPattern,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeRule {
    KeepExisting,
    AddIfAbsent,
    AppendDedup,
    Override,
}

/// (knowledge source, declaration kind, whether the merge target is user-defined) -> merge rule.
pub fn merge_rule(source: SourceKind, decl: DeclKind, target_is_user_defined: bool) -> MergeRule {
    // External sources can't override user-defined `is_module` or an existing superclass.
    let external_overrides_blocked = target_is_user_defined && !source.is_user_source();
    match decl {
        DeclKind::Method => MergeRule::AddIfAbsent,
        DeclKind::Constant => MergeRule::AddIfAbsent,
        DeclKind::TypeParams => MergeRule::AddIfAbsent,
        DeclKind::DirtyPattern => MergeRule::AddIfAbsent,
        DeclKind::Mixin => MergeRule::AppendDedup,
        DeclKind::Ivar => MergeRule::AppendDedup,
        // `merge_external_type_class` only applies this rule when the superclass is absent, so
        // an external declaration fills Ruby's implicit superclass without overriding `class X < Y`.
        DeclKind::Superclass => MergeRule::AddIfAbsent,
        DeclKind::IsModule => {
            if external_overrides_blocked {
                MergeRule::KeepExisting
            } else {
                MergeRule::Override
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXTERNAL_SOURCES: &[SourceKind] = &[
        SourceKind::RbsDecl,
        SourceKind::RbiGenerated,
        SourceKind::RbiHandwritten,
        SourceKind::Schema,
        SourceKind::Plugin,
    ];

    #[test]
    fn ruby_source_is_the_only_user_source() {
        assert!(SourceKind::RubySource.is_user_source());
        for &s in EXTERNAL_SOURCES {
            assert!(!s.is_user_source(), "{s:?} must not be a user source");
        }
    }

    #[test]
    fn method_is_add_if_absent_regardless_of_source_or_target() {
        for &user in &[false, true] {
            assert_eq!(
                merge_rule(SourceKind::RubySource, DeclKind::Method, user),
                MergeRule::AddIfAbsent
            );
            for &s in EXTERNAL_SOURCES {
                assert_eq!(
                    merge_rule(s, DeclKind::Method, user),
                    MergeRule::AddIfAbsent
                );
            }
        }
    }

    #[test]
    fn constant_is_add_if_absent_regardless_of_source_or_target() {
        for &user in &[false, true] {
            assert_eq!(
                merge_rule(SourceKind::RubySource, DeclKind::Constant, user),
                MergeRule::AddIfAbsent
            );
            for &s in EXTERNAL_SOURCES {
                assert_eq!(
                    merge_rule(s, DeclKind::Constant, user),
                    MergeRule::AddIfAbsent
                );
            }
        }
    }

    #[test]
    fn dirty_pattern_is_add_if_absent_regardless_of_source_or_target() {
        for &user in &[false, true] {
            assert_eq!(
                merge_rule(SourceKind::RubySource, DeclKind::DirtyPattern, user),
                MergeRule::AddIfAbsent
            );
            for &s in EXTERNAL_SOURCES {
                assert_eq!(
                    merge_rule(s, DeclKind::DirtyPattern, user),
                    MergeRule::AddIfAbsent
                );
            }
        }
    }

    #[test]
    fn type_params_are_add_if_absent_regardless_of_source_or_target() {
        for &user in &[false, true] {
            assert_eq!(
                merge_rule(SourceKind::RubySource, DeclKind::TypeParams, user),
                MergeRule::AddIfAbsent
            );
            for &s in EXTERNAL_SOURCES {
                assert_eq!(
                    merge_rule(s, DeclKind::TypeParams, user),
                    MergeRule::AddIfAbsent
                );
            }
        }
    }

    #[test]
    fn mixin_is_append_dedup_regardless_of_source_or_target() {
        for &user in &[false, true] {
            assert_eq!(
                merge_rule(SourceKind::RubySource, DeclKind::Mixin, user),
                MergeRule::AppendDedup
            );
            for &s in EXTERNAL_SOURCES {
                assert_eq!(merge_rule(s, DeclKind::Mixin, user), MergeRule::AppendDedup);
            }
        }
    }

    #[test]
    fn ivar_is_append_dedup_regardless_of_source_or_target() {
        for &user in &[false, true] {
            assert_eq!(
                merge_rule(SourceKind::RubySource, DeclKind::Ivar, user),
                MergeRule::AppendDedup
            );
            for &s in EXTERNAL_SOURCES {
                assert_eq!(merge_rule(s, DeclKind::Ivar, user), MergeRule::AppendDedup);
            }
        }
    }

    #[test]
    fn superclass_is_add_if_absent_regardless_of_source_or_target() {
        for &s in EXTERNAL_SOURCES {
            assert_eq!(
                merge_rule(s, DeclKind::Superclass, true),
                MergeRule::AddIfAbsent
            );
            assert_eq!(
                merge_rule(s, DeclKind::Superclass, false),
                MergeRule::AddIfAbsent
            );
        }
    }

    #[test]
    fn superclass_ruby_source_is_add_if_absent_even_on_user_defined_target() {
        assert_eq!(
            merge_rule(SourceKind::RubySource, DeclKind::Superclass, true),
            MergeRule::AddIfAbsent
        );
        assert_eq!(
            merge_rule(SourceKind::RubySource, DeclKind::Superclass, false),
            MergeRule::AddIfAbsent
        );
    }

    #[test]
    fn is_module_external_keeps_existing_only_on_user_defined_target() {
        for &s in EXTERNAL_SOURCES {
            assert_eq!(
                merge_rule(s, DeclKind::IsModule, true),
                MergeRule::KeepExisting
            );
            assert_eq!(
                merge_rule(s, DeclKind::IsModule, false),
                MergeRule::Override
            );
        }
    }

    #[test]
    fn is_module_ruby_source_is_override_even_on_user_defined_target() {
        assert_eq!(
            merge_rule(SourceKind::RubySource, DeclKind::IsModule, true),
            MergeRule::Override
        );
        assert_eq!(
            merge_rule(SourceKind::RubySource, DeclKind::IsModule, false),
            MergeRule::Override
        );
    }
}
