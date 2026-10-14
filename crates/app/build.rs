fn main() {
    // The default skin is compiled in; user skins will load at runtime through slint-interpreter.
    slint_build::compile_with_config(
        "ui/main.slint",
        slint_build::CompilerConfiguration::new().with_style("fluent-dark".into()),
    )
    .expect("compile default skin");
}
