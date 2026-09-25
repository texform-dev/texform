pub fn combinations() -> Vec<String> {
    let mut inputs: Vec<String> = [
        r"{\rm{ }}",
        r"T_{\rm{eff}}",
        r"\rm{ab}",
        "x",
        "+",
        r"\prime\prime",
        r"\not\vb{=}",
        r"\enspace\vb{\enspace}",
        r"\displaystyle x",
        r"\bf x",
        r"\large x",
        r"\mathrm{x}",
        r"\mathrel{=}",
        r"\frac{x}{y}",
        "",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    for input in inputs.clone() {
        for (prefix, suffix) in [
            ("{", "}"),
            ("{{", "}}"),
            (r"\mathbf{", "}"),
            (r"\text{", "}"),
            (r"\textbf{", "}"),
            (r"x^{", "}"),
            (r"\underline{", "}"),
            (r"\mathrel{", "}"),
            (r"\dots{", "}"),
            (r"\operatorname{", "}"),
        ] {
            inputs.push(format!("{prefix}{input}{suffix}"));
        }
    }
    for input in inputs.clone() {
        for (prefix, suffix) in [
            ("{", "}"),
            (r"\mathbf{", "}"),
            (r"\textbf{", "}"),
            (r"\mathbf{\large ", "}"),
            (r"\displaystyle{", "}"),
            (r"\mathrel{", "}"),
            (r"\dots{", "}"),
            (r"\not{", "}"),
            (r"\text{ a ", " b }"),
            (r"\begin{array}{cc}", r" & y\end{array}"),
        ] {
            inputs.push(format!("{prefix}{input}{suffix}"));
        }
    }
    inputs
}
