use crate::rbs::ir as rbs_ir;
use std::hash::{Hash, Hasher};

use rustc_hash::FxHasher;

use crate::inference::FileAnalysisSnapshot;
use crate::registry::{
    CallSite, ClassData, ConstantDef, MethodBlockMeta, MethodBodySummary, MethodDef, Mixin,
    OverloadDef, ParamInfo, TypeRegistry,
};
use crate::types::{Sym, Type};

pub(super) fn hash_u64<T: Hash>(value: &T) -> u64 {
    let mut hasher = FxHasher::default();
    value.hash(&mut hasher);
    hasher.finish()
}

fn hash_param_info(param: &ParamInfo) -> u64 {
    let mut hasher = FxHasher::default();
    param.name.hash(&mut hasher);
    param.kind.hash(&mut hasher);
    param.default_type.hash(&mut hasher);
    hasher.finish()
}

fn hash_rbs_record_key(key: &rbs_ir::RbsRecordKey) -> u64 {
    let mut hasher = FxHasher::default();
    std::mem::discriminant(key).hash(&mut hasher);
    match key {
        rbs_ir::RbsRecordKey::Symbol(name) | rbs_ir::RbsRecordKey::String(name) => {
            name.hash(&mut hasher);
        }
    }
    hasher.finish()
}

fn hash_rbs_type(ty: &rbs_ir::RbsType) -> u64 {
    let mut hasher = FxHasher::default();
    std::mem::discriminant(ty).hash(&mut hasher);
    match ty {
        rbs_ir::RbsType::Class(name, args) | rbs_ir::RbsType::Alias(name, args) => {
            name.hash(&mut hasher);
            for arg in args {
                hash_rbs_type(arg).hash(&mut hasher);
            }
        }
        rbs_ir::RbsType::Singleton(name) | rbs_ir::RbsType::Variable(name) => {
            name.hash(&mut hasher);
        }
        rbs_ir::RbsType::Literal(name) => {
            name.hash(&mut hasher);
        }
        rbs_ir::RbsType::Union(parts)
        | rbs_ir::RbsType::Intersection(parts)
        | rbs_ir::RbsType::Tuple(parts) => {
            for part in parts {
                hash_rbs_type(part).hash(&mut hasher);
            }
        }
        rbs_ir::RbsType::Optional(inner) => {
            hash_rbs_type(inner).hash(&mut hasher);
        }
        rbs_ir::RbsType::Record(fields) => {
            for field in fields {
                hash_rbs_record_key(&field.key).hash(&mut hasher);
                hash_rbs_type(&field.type_).hash(&mut hasher);
                field.required.hash(&mut hasher);
            }
        }
        rbs_ir::RbsType::Proc(method_type) => {
            hash_rbs_method_type(method_type).hash(&mut hasher);
        }
        _ => {}
    }
    hasher.finish()
}

fn hash_function_param(param: &rbs_ir::FunctionParam) -> u64 {
    let mut hasher = FxHasher::default();
    hash_rbs_type(&param.type_).hash(&mut hasher);
    param.name.hash(&mut hasher);
    hasher.finish()
}

fn hash_function_type(function_type: &rbs_ir::FunctionType) -> u64 {
    let mut hasher = FxHasher::default();
    combine_unordered_hashes(
        function_type
            .required_positionals
            .iter()
            .map(hash_function_param),
    )
    .hash(&mut hasher);
    combine_unordered_hashes(
        function_type
            .optional_positionals
            .iter()
            .map(hash_function_param),
    )
    .hash(&mut hasher);
    function_type
        .rest_positionals
        .as_deref()
        .map(hash_function_param)
        .hash(&mut hasher);
    combine_unordered_hashes(
        function_type
            .trailing_positionals
            .iter()
            .map(hash_function_param),
    )
    .hash(&mut hasher);
    for (keyword, param) in &function_type.required_keywords {
        keyword.hash(&mut hasher);
        hash_function_param(param).hash(&mut hasher);
    }
    for (keyword, param) in &function_type.optional_keywords {
        keyword.hash(&mut hasher);
        hash_function_param(param).hash(&mut hasher);
    }
    function_type
        .rest_keywords
        .as_deref()
        .map(hash_function_param)
        .hash(&mut hasher);
    hash_rbs_type(&function_type.return_type).hash(&mut hasher);
    hasher.finish()
}

fn hash_block_type(block: &rbs_ir::BlockType) -> u64 {
    let mut hasher = FxHasher::default();
    hash_function_type(&block.function_type).hash(&mut hasher);
    block.required.hash(&mut hasher);
    block
        .self_type
        .as_deref()
        .map(hash_rbs_type)
        .hash(&mut hasher);
    hasher.finish()
}

fn hash_rbs_method_type(method_type: &rbs_ir::MethodType) -> u64 {
    let mut hasher = FxHasher::default();
    hash_function_type(&method_type.function_type).hash(&mut hasher);
    method_type
        .self_type
        .as_deref()
        .map(hash_rbs_type)
        .hash(&mut hasher);
    method_type
        .block
        .as_deref()
        .map(hash_block_type)
        .hash(&mut hasher);
    method_type.type_params.hash(&mut hasher);
    for (name, bound) in &method_type.type_param_bounds {
        name.hash(&mut hasher);
        hash_rbs_type(bound).hash(&mut hasher);
    }
    for (name, lower_bound) in &method_type.type_param_lower_bounds {
        name.hash(&mut hasher);
        hash_rbs_type(lower_bound).hash(&mut hasher);
    }
    method_type.annotations.hash(&mut hasher);
    hasher.finish()
}

fn hash_overload(overload: &OverloadDef) -> u64 {
    let mut hasher = FxHasher::default();
    overload.param_types.hash(&mut hasher);
    overload.return_type.hash(&mut hasher);
    hasher.finish()
}

fn hash_mixin(mixin: &Mixin) -> u64 {
    let mut hasher = FxHasher::default();
    mixin.module_name.hash(&mut hasher);
    for arg in &mixin.type_args {
        hash_rbs_type(arg).hash(&mut hasher);
    }
    std::mem::discriminant(&mixin.kind).hash(&mut hasher);
    hasher.finish()
}

fn hash_required_ancestor_type_args(
    entry: &(crate::types::SharedName, Vec<rbs_ir::RbsType>),
) -> u64 {
    let mut hasher = FxHasher::default();
    entry.0.hash(&mut hasher);
    for arg in &entry.1 {
        hash_rbs_type(arg).hash(&mut hasher);
    }
    hasher.finish()
}

fn hash_class_type_param_bound(entry: &(String, rbs_ir::RbsType)) -> u64 {
    let mut hasher = FxHasher::default();
    entry.0.hash(&mut hasher);
    hash_rbs_type(&entry.1).hash(&mut hasher);
    hasher.finish()
}

fn hash_export_method(method: &MethodDef) -> u64 {
    let mut hasher = FxHasher::default();
    method.name.hash(&mut hasher);
    method.is_singleton.hash(&mut hasher);
    method.raw_return_type.hash(&mut hasher);
    for param in &method.param_infos {
        param.name.hash(&mut hasher);
        param.kind.hash(&mut hasher);
        param.default_type.hash(&mut hasher);
    }
    hasher.finish()
}

fn hash_registry_method(method: &MethodDef) -> u64 {
    let mut hasher = FxHasher::default();
    method.name.hash(&mut hasher);
    combine_unordered_hashes(method.param_infos.iter().map(hash_param_info)).hash(&mut hasher);
    method.raw_return_type.hash(&mut hasher);
    method.rbs_annotated.hash(&mut hasher);
    method.rbs_inline_annotated.hash(&mut hasher);
    method.sig_annotated.hash(&mut hasher);
    method.attr_ivar.hash(&mut hasher);
    method.is_singleton.hash(&mut hasher);
    method.rbs_file_source.hash(&mut hasher);
    method.synthetic_dsl_source.hash(&mut hasher);
    combine_unordered_hashes(method.extra_overloads.iter().map(hash_overload)).hash(&mut hasher);
    for method_type in method.rbs_method_types.iter() {
        hash_rbs_method_type(method_type).hash(&mut hasher);
    }
    method.loc.hash(&mut hasher);
    hasher.finish()
}

fn hash_export_constant(entry: (&Sym, &ConstantDef)) -> u64 {
    let mut hasher = FxHasher::default();
    entry.0.hash(&mut hasher);
    entry.1.const_type.hash(&mut hasher);
    hasher.finish()
}

fn hash_registry_constant(entry: (&Sym, &ConstantDef)) -> u64 {
    let mut hasher = FxHasher::default();
    entry.0.hash(&mut hasher);
    entry.1.const_type.hash(&mut hasher);
    entry.1.loc.hash(&mut hasher);
    entry.1.file_path.hash(&mut hasher);
    hasher.finish()
}

fn hash_ivar(entry: (&Sym, &Vec<Type>)) -> u64 {
    let mut hasher = FxHasher::default();
    entry.0.hash(&mut hasher);
    entry.1.hash(&mut hasher);
    hasher.finish()
}

fn hash_call_site(call_site: &CallSite) -> u64 {
    hash_u64(call_site)
}

fn hash_method_block_meta(meta: &MethodBlockMeta) -> u64 {
    let mut hasher = FxHasher::default();
    meta.yield_param_types.hash(&mut hasher);
    meta.forwarded_block.hash(&mut hasher);
    hasher.finish()
}

fn hash_method_block_meta_map(
    entries: &std::collections::HashMap<crate::types::SharedName, MethodBlockMeta>,
) -> (u64, u64, u64, u64) {
    combine_unordered_hashes(entries.iter().map(|(method_name, meta)| {
        let mut hasher = FxHasher::default();
        method_name.hash(&mut hasher);
        hash_method_block_meta(meta).hash(&mut hasher);
        hasher.finish()
    }))
}

fn hash_method_body_summary(summary: &MethodBodySummary) -> u64 {
    let call_sites = combine_unordered_hashes(summary.call_sites_by_class.iter().map(
        |(class_name, sites)| {
            let mut hasher = FxHasher::default();
            class_name.hash(&mut hasher);
            combine_unordered_hashes(sites.iter().map(hash_call_site)).hash(&mut hasher);
            hasher.finish()
        },
    ));
    let ivar_types = combine_unordered_hashes(summary.ivar_types_by_class.iter().map(
        |(class_name, ivars)| {
            let mut hasher = FxHasher::default();
            class_name.hash(&mut hasher);
            combine_unordered_hashes(ivars.iter().map(|(ivar_name, types)| {
                let mut hasher = FxHasher::default();
                ivar_name.hash(&mut hasher);
                types.hash(&mut hasher);
                hasher.finish()
            }))
            .hash(&mut hasher);
            hasher.finish()
        },
    ));
    let method_meta = combine_unordered_hashes(summary.method_block_meta_by_class.iter().map(
        |(class_name, class_meta)| {
            let mut hasher = FxHasher::default();
            class_name.hash(&mut hasher);
            hash_method_block_meta_map(&class_meta.instance).hash(&mut hasher);
            hash_method_block_meta_map(&class_meta.singleton).hash(&mut hasher);
            hasher.finish()
        },
    ));
    hash_u64(&(call_sites, ivar_types, method_meta))
}

fn hash_class_data_fingerprints(class_name: &str, data: &ClassData) -> (u64, u64) {
    let mixins = combine_unordered_hashes(data.mixins.iter().map(hash_mixin));
    let required_ancestors =
        combine_unordered_hashes(data.cold().required_ancestors.iter().map(hash_u64));
    let required_ancestor_type_args = combine_unordered_hashes(
        data.cold()
            .required_ancestor_type_args
            .iter()
            .map(hash_required_ancestor_type_args),
    );
    let export_methods =
        combine_unordered_hashes(data.methods.iter().map(|m| hash_export_method(m)));
    let registry_methods =
        combine_unordered_hashes(data.methods.iter().map(|m| hash_registry_method(m)));
    let export_constants =
        combine_unordered_hashes(data.constants.iter().map(hash_export_constant));
    let registry_constants =
        combine_unordered_hashes(data.constants.iter().map(hash_registry_constant));
    let ivars = combine_unordered_hashes(data.ivars.iter().map(hash_ivar));
    let singleton_ivars =
        combine_unordered_hashes(data.cold().singleton_ivars.iter().map(hash_ivar));
    let class_variables =
        combine_unordered_hashes(data.cold().class_variables.iter().map(hash_ivar));

    let export = {
        let mut hasher = FxHasher::default();
        class_name.hash(&mut hasher);
        data.superclass.hash(&mut hasher);
        for arg in &data.cold().superclass_type_args {
            hash_rbs_type(arg).hash(&mut hasher);
        }
        data.is_module.hash(&mut hasher);
        mixins.hash(&mut hasher);
        required_ancestors.hash(&mut hasher);
        required_ancestor_type_args.hash(&mut hasher);
        export_methods.hash(&mut hasher);
        export_constants.hash(&mut hasher);
        ivars.hash(&mut hasher);
        singleton_ivars.hash(&mut hasher);
        class_variables.hash(&mut hasher);
        hasher.finish()
    };

    let registry = {
        let mut hasher = FxHasher::default();
        class_name.hash(&mut hasher);
        data.superclass.hash(&mut hasher);
        for arg in &data.cold().superclass_type_args {
            hash_rbs_type(arg).hash(&mut hasher);
        }
        mixins.hash(&mut hasher);
        required_ancestors.hash(&mut hasher);
        required_ancestor_type_args.hash(&mut hasher);
        registry_methods.hash(&mut hasher);
        registry_constants.hash(&mut hasher);
        ivars.hash(&mut hasher);
        singleton_ivars.hash(&mut hasher);
        class_variables.hash(&mut hasher);
        combine_unordered_hashes(data.call_sites.iter().map(hash_call_site)).hash(&mut hasher);
        combine_unordered_hashes(data.cold().annotated_params.iter().flat_map(
            |((method_name, is_singleton), params)| {
                params.iter().map(move |(idx, ty)| {
                    let mut hasher = FxHasher::default();
                    method_name.hash(&mut hasher);
                    is_singleton.hash(&mut hasher);
                    idx.hash(&mut hasher);
                    ty.hash(&mut hasher);
                    hasher.finish()
                })
            },
        ))
        .hash(&mut hasher);
        data.is_module.hash(&mut hasher);
        data.cold().class_type_params.hash(&mut hasher);
        combine_unordered_hashes(
            data.cold()
                .class_type_param_bounds
                .iter()
                .map(hash_class_type_param_bound),
        )
        .hash(&mut hasher);
        data.cold().class_type_param_defaults.hash(&mut hasher);
        data.loc.hash(&mut hasher);
        data.file_path.hash(&mut hasher);
        hasher.finish()
    };

    (export, registry)
}

pub(super) fn combine_unordered_hashes<I>(iter: I) -> (u64, u64, u64, u64)
where
    I: IntoIterator<Item = u64>,
{
    let mut sum = 0u64;
    let mut xor = 0u64;
    let mut product = 1u64;
    let mut count = 0u64;
    for hash in iter {
        sum = sum.wrapping_add(hash);
        xor ^= hash.rotate_left(((hash >> 58) as u32) + 1);
        product = product.wrapping_mul(hash.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        count += 1;
    }
    (sum, xor, product, count)
}

const UNORDERED_HASH_PRODUCT_SALT: u64 = 0x9E37_79B9_7F4A_7C15;

fn rotated_unordered_hash(hash: u64) -> u64 {
    hash.rotate_left(((hash >> 58) as u32) + 1)
}

fn encoded_unordered_hash(hash: u64) -> u64 {
    hash.wrapping_mul(UNORDERED_HASH_PRODUCT_SALT) | 1
}

fn invert_odd_u64(value: u64) -> u64 {
    debug_assert_eq!(value & 1, 1, "only odd values are invertible modulo 2^64");
    let mut inverse = 1u64;
    for _ in 0..6 {
        inverse = inverse.wrapping_mul(2u64.wrapping_sub(value.wrapping_mul(inverse)));
    }
    inverse
}

#[derive(Debug, Clone, Copy)]
pub(super) struct FingerprintAggregate {
    sum: u64,
    xor: u64,
    product: u64,
    count: u64,
}

impl Default for FingerprintAggregate {
    fn default() -> Self {
        Self {
            sum: 0,
            xor: 0,
            product: 1,
            count: 0,
        }
    }
}

impl FingerprintAggregate {
    fn from_hashes(iter: impl IntoIterator<Item = u64>) -> Self {
        let mut aggregate = Self::default();
        for hash in iter {
            aggregate.add_hash(hash);
        }
        aggregate
    }

    fn as_tuple(self) -> (u64, u64, u64, u64) {
        (self.sum, self.xor, self.product, self.count)
    }

    pub(super) fn add_hash(&mut self, hash: u64) {
        self.sum = self.sum.wrapping_add(hash);
        self.xor ^= rotated_unordered_hash(hash);
        self.product = self.product.wrapping_mul(encoded_unordered_hash(hash));
        self.count += 1;
    }

    pub(super) fn remove_hash(&mut self, hash: u64) {
        debug_assert!(self.count > 0, "cannot remove from empty aggregate");
        self.sum = self.sum.wrapping_sub(hash);
        self.xor ^= rotated_unordered_hash(hash);
        let encoded = encoded_unordered_hash(hash);
        self.product = self.product.wrapping_mul(invert_odd_u64(encoded));
        self.count -= 1;
    }

    pub(super) fn fingerprint_excluding(&self, excluded_hash: Option<u64>) -> u64 {
        let mut aggregate = *self;
        if let Some(hash) = excluded_hash {
            aggregate.remove_hash(hash);
        }
        hash_u64(&aggregate.as_tuple())
    }
}

/// Export fingerprint: no downstream reanalysis needed if only the body changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExportFingerprint(pub(crate) u64);

impl ExportFingerprint {
    pub fn from_registry(registry: &TypeRegistry) -> Self {
        let class_hashes = registry
            .iter_class_data()
            .map(|(class_name, data)| hash_class_data_fingerprints(class_name.as_str(), data).0);

        Self(hash_u64(&combine_unordered_hashes(class_hashes)))
    }
}

/// Merge fingerprint: also updates the merged registry cache on body changes (broader than Export).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RegistryFingerprint(pub(crate) u64);

impl RegistryFingerprint {
    pub fn from_analysis(analysis: &FileAnalysisSnapshot) -> Self {
        let registry = analysis.registry();
        let class_hashes = combine_unordered_hashes(
            registry
                .iter_class_data()
                .map(|(name, data)| hash_class_data_fingerprints(name.as_str(), data).1),
        );
        let type_aliases =
            combine_unordered_hashes(registry.type_aliases().iter().map(|(alias_name, ty)| {
                let mut hasher = FxHasher::default();
                alias_name.hash(&mut hasher);
                ty.hash(&mut hasher);
                hasher.finish()
            }));
        let method_body_summary = hash_method_body_summary(&analysis.method_body_summary);
        Self(hash_u64(&(class_hashes, type_aliases, method_body_summary)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileFingerprints {
    pub export: ExportFingerprint,
    pub registry: RegistryFingerprint,
}

impl FileFingerprints {
    pub fn from_analysis(analysis: &FileAnalysisSnapshot) -> Self {
        let registry = analysis.registry();
        let mut export_class_hashes = FingerprintAggregate::default();
        let mut registry_class_hashes = FingerprintAggregate::default();
        for (class_name, data) in registry.iter_class_data() {
            let (export_hash, registry_hash) =
                hash_class_data_fingerprints(class_name.as_str(), data);
            export_class_hashes.add_hash(export_hash);
            registry_class_hashes.add_hash(registry_hash);
        }
        let export = ExportFingerprint(hash_u64(&export_class_hashes.as_tuple()));
        let type_aliases = FingerprintAggregate::from_hashes(registry.type_aliases().iter().map(
            |(alias_name, ty)| {
                let mut hasher = FxHasher::default();
                alias_name.hash(&mut hasher);
                ty.hash(&mut hasher);
                hasher.finish()
            },
        ));
        let method_body_summary = hash_method_body_summary(&analysis.method_body_summary);
        let registry = RegistryFingerprint(hash_u64(&(
            registry_class_hashes.as_tuple(),
            type_aliases.as_tuple(),
            method_body_summary,
        )));
        Self { export, registry }
    }
}

pub fn hash_content(source: &str) -> u64 {
    let mut hasher = FxHasher::default();
    source.hash(&mut hasher);
    hasher.finish()
}
