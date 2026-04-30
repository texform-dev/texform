pub(crate) fn is_valid_dimension_unit(unit: &str) -> bool {
    matches!(
        unit,
        "em" | "ex" | "pt" | "pc" | "px" | "in" | "cm" | "mm" | "mu"
    )
}
