use super::*;
use crate::dep_graph::DepEdge;

impl<'a> InferenceEngine<'a> {
    /// Records the symbols that the file's classes name and does not define. Call after
    /// parameter resolution and receiver preload.
    pub fn finalize_deps(&mut self) {
        let deps = &mut self.file_deps;
        let defined = &deps.defined_symbols;
        let refs = &mut deps.referenced_symbols;
        let edges = &mut deps.edges;
        let undefined = |symbol: &str| !defined.contains(symbol);
        self.registry.for_each_class_dep_scan(|scan| {
            for symbol in &scan.references {
                if undefined(symbol) && !refs.contains(&**symbol) {
                    refs.insert(symbol.to_string());
                }
            }
            for (symbol, kind) in &scan.edges {
                if undefined(symbol) {
                    edges.push(DepEdge {
                        symbol: symbol.to_string(),
                        kind: *kind,
                    });
                }
            }
            for (symbol, kind) in &scan.inheritance {
                edges.push(DepEdge {
                    symbol: symbol.to_string(),
                    kind: *kind,
                });
            }
        });
        // The graph keeps edges as a set, so repeats only cost memory.
        edges.sort_unstable_by(|a, b| (a.kind, &a.symbol).cmp(&(b.kind, &b.symbol)));
        edges.dedup_by(|a, b| a.kind == b.kind && a.symbol == b.symbol);
    }
}
