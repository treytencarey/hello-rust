use syn::{visit::Visit, ItemFn};
use std::fs;

struct FnVisitor;

impl<'ast> Visit<'ast> for FnVisitor {
    fn visit_item_fn(&mut self, i: &'ast ItemFn) {
        println!("{}", i.sig.ident);
        syn::visit::visit_item_fn(self, i);
    }
}

fn main() {
    let code = fs::read_to_string("src/main.rs").unwrap();
    let syntax_tree = syn::parse_file(&code).unwrap();
    FnVisitor.visit_file(&syntax_tree);
}