use formula_snap_lib::convert::{latex_to_mathml, mathml_to_omml, latex_to_omml};
fn main() {
    let latex = r"A_{k_2}^{s2t}";
    let mathml = latex_to_mathml(latex).unwrap();
    println!("MathML:\n{}", mathml);
    let omml = mathml_to_omml(&mathml).unwrap();
    println!("\nOMML:\n{}", omml);
}
