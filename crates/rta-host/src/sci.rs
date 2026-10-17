//! Software configuration index. Hashes, not branch tips.

#[cfg(test)]
mod tests {
    #[test]
    fn the_index_names_hashes_and_the_swap_updates_it() {
        let index = include_str!("../../../docs/do178/sci.md");
        let bundle = include_str!("../../../docs/reload-bundle.md");
        assert!(index.contains("ae398e85f30cd1e8723bbef36ad934b3651017e6d987eeedbc7fddf229bd6553"));
        assert!(index.contains("d6bdf2dddc6753beede60dc65731cf8bb06a49a355c90eccf802abe181bc2342"));
        assert!(index.contains("8ef36e2c13b88ae726738d52da06723be20e7ad52127db46f0b8b069dfb052ac"));
        assert!(index.contains("not floating branch tips"));
        assert!(!index.contains("branch: main"));
        assert!(index.contains("new index row, not an edit"));
        assert!(index.contains("same change as the swap"));
        assert!(bundle.contains("same change as the swap"));
        assert!(index.contains("rtlola-interpreter"));
        assert!(index.contains("0.11.0"));
    }
}
