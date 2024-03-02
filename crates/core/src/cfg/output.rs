use petgraph::{dot::Dot, graph::Graph};

use super::CFGArgs;

/// Write the generated CFG to a file in the `dot` graphviz format.
pub fn build_cfg(contract_cfg: &Graph<String, String>, args: &CFGArgs) -> String {
    let output = format!("{}", Dot::with_config(&contract_cfg, &[]));

    // find regex matches and replace
    let mut output = output.replace(
        "digraph {",
        "digraph G {\n    node [shape=box, style=\"rounded\", fontname=\"Helvetica\"];\n    edge [fontname=\"Helvetica\"];"
    );

    if args.color_edges {
        // replace edge labels with colors
        output = output.replace("[ label = \"true\" ]", "[ color = \"green\" ]");
        output = output.replace("[ label = \"false\" ]", "[ color = \"red\" ]");
    } else {
        // remove edge labels
        output = output.replace("[ label = \"true\" ]", "[]");
        output = output.replace("[ label = \"false\" ]", "[]");
    }

    output = output.replace("[ label = \"\" ]", "[]");

    output
}
