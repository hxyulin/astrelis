use proc_macro::{Span, TokenStream};
use quote::{ToTokens, TokenStreamExt};

struct ShaderBufferCompatible {
    name: syn::Ident,
    fields: Vec<(syn::Ident, syn::Type)>,
}

impl syn::parse::Parse for ShaderBufferCompatible {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let input: syn::DeriveInput = input.parse()?;

        if let syn::Data::Struct(struc) = input.data {
            let mut fields = Vec::with_capacity(struc.fields.len());

            for field in struc.fields {
                match field.ident {
                    Some(ident) => fields.push((ident, field.ty)),
                    None => return Err(syn::Error::new_spanned(field, "expected a named field")),
                }
            }

            Ok(ShaderBufferCompatible {
                name: input.ident,
                fields,
            })
        } else {
            Err(syn::Error::new_spanned(input.ident, "expected a struct"))
        }
    }
}

impl quote::ToTokens for ShaderBufferCompatible {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let cr = match proc_macro_crate::crate_name("astrelis-core").unwrap() {
            proc_macro_crate::FoundCrate::Itself => quote::quote! { crate },
            proc_macro_crate::FoundCrate::Name(name) => {
                // TODO: Maybe make this a function we get actual call site
                let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
                quote::quote!( ::#ident)
            }
        };

        let attributes = self.fields.iter().enumerate().map(|(i, (ident, ty))| {
            let i = i as u32;
            quote::quote! {
                wgpu::VertexAttribute {
                    offset: ::std::mem::offset_of!(Self, #ident) as u64,
                    shader_location: base_location + #i,
                    format: <#ty as #cr::graphics::shader::AsVertexFormat>::vertex_format(),
                }
            }
        });

        let name = &self.name;

        tokens.append_all(quote::quote! {
            impl #cr::graphics::shader::ShaderBufferCompatible for #name {
                fn buffer_layout(base_location: u32) -> #cr::graphics::shader::BufferLayout {
                    let attributes = vec![#(#attributes),*];

                    #cr::graphics::shader::BufferLayout {
                        attributes,
                        size: ::std::mem::size_of::<Self>() as u64,
                    }
                }
            }
        });
    }
}

#[proc_macro_derive(ShaderBufferCompatible)]
pub fn derive_shader_buffer_compatible(item: TokenStream) -> TokenStream {
    match syn::parse::<ShaderBufferCompatible>(item) {
        Ok(buf) => buf.to_token_stream().into(),
        Err(err) => err.to_compile_error().into(),
    }
}
