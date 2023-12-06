extern crate proc_macro;

use proc_macro::{TokenStream, TokenTree};

#[proc_macro_derive(Serialize)]
pub fn derive_serialize(item: TokenStream) -> TokenStream {
    let mut result = String::default();

    let mut iter = item.into_iter();

    if let TokenTree::Ident(token_type) = iter.next().unwrap() {
        // check if it is struct
        println!("type: {:?}", token_type);
    } else {
        panic!("first token should be ident struct")
    }

    let name = if let TokenTree::Ident(name) = iter.next().unwrap() {
        println!("name: {:?}", name);
        name.to_string()
    } else {
        panic!("second token should be ident name")
    };
    result.push_str("impl Serialize for ");
    result.push_str(name.as_str());
    result.push_str(" {\n");
    result
        .push_str("\tfn serialize(&self, buffer: &mut [u8]) -> Result<usize, SerializeError> {\n");

    if let TokenTree::Group(g) = iter.next().unwrap() {
        let mut var_tokens = g.stream().into_iter();
        result.push_str("\t\tlet mut offset = 0;\n");
        while let Some(TokenTree::Ident(ident)) = var_tokens.next() {
            let var_name = ident.to_string();
            result.push_str(
                format!(
                    "\t\toffset += self.{}.serialize(&mut buffer[offset..])?;\n",
                    var_name
                )
                .as_str(),
            );

            loop {
                match var_tokens.next() {
                    None => break,
                    Some(x) => {
                        match x {
                            TokenTree::Punct(p) => {
                                // TODO bag if 2+ generics in type
                                if p.as_char() == ',' {
                                    break;
                                }
                            }
                            _ => continue,
                        }
                    }
                }
            }
        }
    } else {
        todo!()
    }

    result.push_str("\t\tOk(offset)\n");
    result.push_str("\t}\n");
    result.push_str("}");

    println!("final result: \n'{}'", result);

    result.parse().unwrap()
}
