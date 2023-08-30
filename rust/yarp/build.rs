#![allow(
    clippy::too_many_lines,
    clippy::uninlined_format_args
)]

use serde::Deserialize;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
enum NodeFieldType {
    #[serde(rename = "node")]
    Node,

    #[serde(rename = "node?")]
    OptionalNode,

    #[serde(rename = "node[]")]
    NodeList,

    #[serde(rename = "string")]
    String,

    #[serde(rename = "constant")]
    Constant,

    #[serde(rename = "constant[]")]
    ConstantList,

    #[serde(rename = "location")]
    Location,

    #[serde(rename = "location?")]
    OptionalLocation,

    #[serde(rename = "location[]")]
    LocationList,

    #[serde(rename = "uint32")]
    UInt32,

    #[serde(rename = "flags")]
    Flags
}

#[derive(Debug, Deserialize)]
struct NodeField {
    name: String,

    #[serde(rename = "type")]
    field_type: NodeFieldType,

    kind: Option<String>
}

#[derive(Debug, Deserialize)]
struct Node {
    name: String,

    #[serde(default)]
    fields: Vec<NodeField>,

    comment: String
}

#[derive(Debug, Deserialize)]
struct Config {
    nodes: Vec<Node>
}

/// The main function for the build script. This will be run by Cargo when
/// building the library.
/// 
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = config_path();
    let file = std::fs::File::open(&path)?;
    println!("cargo:rerun-if-changed={}", path.to_str().unwrap());
    let config: Config = serde_yaml::from_reader(file)?;

    write_bindings(&config)?;
    Ok(())
}

/// Gets the path to the config.yml file at `[root]/config.yml`.
///
fn config_path() -> PathBuf {
    cargo_manifest_path()
        .join("../../config.yml")
        .canonicalize()
        .unwrap()
}

/// Gets the path to the root directory at `[root]`.
/// 
fn cargo_manifest_path() -> PathBuf {
    PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
}

/// Returns the name of a C struct from the given node name.
fn struct_name(name: &str) -> String {
    let mut result = String::with_capacity(1 + name.len());

    for char in name.chars() {
        if char.is_uppercase() {
            result.push('_');
        }
        result.push(char.to_lowercase().next().unwrap());
    }

    result
}

/// Returns the name of the C type from the given node name.
fn type_name(name: &str) -> String {
    let mut result = String::with_capacity(8 + name.len());
    result.push_str("YP_NODE");

    for char in name.chars() {
        if char.is_uppercase() {
            result.push('_');
        }
        result.push(char.to_uppercase().next().unwrap());
    }

    result
}

/// Write the generated struct for the node to the file.
fn write_node(file: &mut File, node: &Node) -> Result<(), Box<dyn std::error::Error>> {
    let mut example = false;

    for line in node.comment.lines() {
        if let Some(stripped) = line.strip_prefix("    ") {
            if !example {
                writeln!(file, "/// ```ruby")?;
                example = true;
            }
            writeln!(file, "/// {}", stripped)?;
        } else {
            if example {
                writeln!(file, "/// ```")?;
                example = false;
            }
            writeln!(file, "/// {}", line)?;
        }
    }

    if example {
        writeln!(file, "/// ```")?;
    }

    writeln!(file, "pub struct {}<'pr> {{", node.name)?;
    writeln!(file, "    /// The pointer to the parser this node came from.")?;
    writeln!(file, "    parser: NonNull<yp_parser_t>,")?;
    writeln!(file)?;
    writeln!(file, "    /// The raw pointer to the node allocated by YARP.")?;
    writeln!(file, "    pointer: *mut yp{}_t,", struct_name(&node.name))?;
    writeln!(file)?;
    writeln!(file, "    /// The marker to indicate the lifetime of the pointer.")?;
    writeln!(file, "    marker: PhantomData<&'pr mut yp{}_t>", struct_name(&node.name))?;
    writeln!(file, "}}")?;
    writeln!(file)?;
    writeln!(file, "impl<'pr> {}<'pr> {{", node.name)?;
    writeln!(file, "    /// Converts this node to a generic node.")?;
    writeln!(file, "    #[must_use]")?;
    writeln!(file, "    pub fn as_node(&self) -> Node<'pr> {{")?;
    writeln!(file, "        Node::{} {{ parser: self.parser, pointer: self.pointer, marker: PhantomData }}", node.name)?;
    writeln!(file, "    }}")?;
    writeln!(file)?;
    writeln!(file, "    /// Returns the location of this node.")?;
    writeln!(file, "    #[must_use]")?;
    writeln!(file, "    pub fn location(&self) -> Location<'pr> {{")?;
    writeln!(file, "        let pointer: *mut yp_location_t = unsafe {{ &mut (*self.pointer).base.location }};")?;
    writeln!(file, "        Location {{ pointer: unsafe {{ NonNull::new_unchecked(pointer) }}, marker: PhantomData }}")?;
    writeln!(file, "    }}")?;

    for field in &node.fields {
        writeln!(file)?;
        writeln!(file, "    /// Returns the `{}` param", field.name)?;
        writeln!(file, "    #[must_use]")?;

        match field.field_type {
            NodeFieldType::Node => {
                if let Some(kind) = &field.kind {
                    writeln!(file, "    pub fn {}(&self) -> {}<'pr> {{", field.name, kind)?;
                    writeln!(file, "        let node: *mut yp{}_t = unsafe {{ (*self.pointer).{} }};", struct_name(kind), field.name)?;
                    writeln!(file, "        {} {{ parser: self.parser, pointer: node, marker: PhantomData }}", kind)?;
                    writeln!(file, "    }}")?;
                } else {
                    writeln!(file, "    pub fn {}(&self) -> Node<'pr> {{", field.name)?;
                    writeln!(file, "        let node: *mut yp_node_t = unsafe {{ (*self.pointer).{} }};", field.name)?;
                    writeln!(file, "        Node::new(self.parser, node)")?;
                    writeln!(file, "    }}")?;
                }
            },
            NodeFieldType::OptionalNode => {
                if let Some(kind) = &field.kind {
                    writeln!(file, "    pub fn {}(&self) -> Option<{}<'pr>> {{", field.name, kind)?;
                    writeln!(file, "        let node: *mut yp{}_t = unsafe {{ (*self.pointer).{} }};", struct_name(kind), field.name)?;
                    writeln!(file, "        if node.is_null() {{")?;
                    writeln!(file, "            None")?;
                    writeln!(file, "        }} else {{")?;
                    writeln!(file, "            Some({} {{ parser: self.parser, pointer: node, marker: PhantomData }})", kind)?;
                    writeln!(file, "        }}")?;
                    writeln!(file, "    }}")?;
                } else {
                    writeln!(file, "    pub fn {}(&self) -> Option<Node<'pr>> {{", field.name)?;
                    writeln!(file, "        let node: *mut yp_node_t = unsafe {{ (*self.pointer).{} }};", field.name)?;
                    writeln!(file, "        if node.is_null() {{")?;
                    writeln!(file, "            None")?;
                    writeln!(file, "        }} else {{")?;
                    writeln!(file, "            Some(Node::new(self.parser, node))")?;
                    writeln!(file, "        }}")?;
                    writeln!(file, "    }}")?;
                }
            },
            NodeFieldType::NodeList => {
                writeln!(file, "    pub fn {}(&self) -> NodeList<'pr> {{", field.name)?;
                writeln!(file, "        let pointer: *mut yp_node_list = unsafe {{ &mut (*self.pointer).{} }};", field.name)?;
                writeln!(file, "        NodeList {{ parser: self.parser, pointer: unsafe {{ NonNull::new_unchecked(pointer) }}, marker: PhantomData }}")?;
                writeln!(file, "    }}")?;
            },
            NodeFieldType::String => {
                writeln!(file, "    pub const fn {}(&self) -> &str {{", field.name)?;
                writeln!(file, "        \"\"")?;
                writeln!(file, "    }}")?;
            },
            NodeFieldType::Constant => {
                writeln!(file, "    pub fn {}(&self) -> ConstantId<'pr> {{", field.name)?;
                writeln!(file, "        ConstantId::new(self.parser, unsafe {{ (*self.pointer).{} }})", field.name)?;
                writeln!(file, "    }}")?;
            },
            NodeFieldType::ConstantList => {
                writeln!(file, "    pub fn {}(&self) -> ConstantList<'pr> {{", field.name)?;
                writeln!(file, "        let pointer: *mut yp_constant_id_list_t = unsafe {{ &mut (*self.pointer).{} }};", field.name)?;
                writeln!(file, "        ConstantList {{ parser: self.parser, pointer: unsafe {{ NonNull::new_unchecked(pointer) }}, marker: PhantomData }}")?;
                writeln!(file, "    }}")?;
            },
            NodeFieldType::Location => {
                writeln!(file, "    pub fn {}(&self) -> Location<'pr> {{", field.name)?;
                writeln!(file, "        let pointer: *mut yp_location_t = unsafe {{ &mut (*self.pointer).{} }};", field.name)?;
                writeln!(file, "        Location {{ pointer: unsafe {{ NonNull::new_unchecked(pointer) }}, marker: PhantomData }}")?;
                writeln!(file, "    }}")?;
            },
            NodeFieldType::OptionalLocation => {
                writeln!(file, "    pub fn {}(&self) -> Option<Location<'pr>> {{", field.name)?;
                writeln!(file, "        let pointer: *mut yp_location_t = unsafe {{ &mut (*self.pointer).{} }};", field.name)?;
                writeln!(file, "        if pointer.is_null() {{")?;
                writeln!(file, "            None")?;
                writeln!(file, "        }} else {{")?;
                writeln!(file, "            Some(Location {{ pointer: unsafe {{ NonNull::new_unchecked(pointer) }}, marker: PhantomData }})")?;
                writeln!(file, "        }}")?;
                writeln!(file, "    }}")?;
            },
            NodeFieldType::LocationList => {
                writeln!(file, "    pub fn {}(&self) -> LocationList<'pr> {{", field.name)?;
                writeln!(file, "        let pointer: *mut yp_location_list_t = unsafe {{ &mut (*self.pointer).{} }};", field.name)?;
                writeln!(file, "        LocationList {{ pointer: unsafe {{ NonNull::new_unchecked(pointer) }}, marker: PhantomData }}")?;
                writeln!(file, "    }}")?;
            },
            NodeFieldType::UInt32 => {
                writeln!(file, "    pub fn {}(&self) -> u32 {{", field.name)?;
                writeln!(file, "        unsafe {{ (*self.pointer).{} }}", field.name)?;
                writeln!(file, "    }}")?;
            },
            NodeFieldType::Flags => {
                writeln!(file, "    pub fn {}(&self) -> yp_node_flags_t {{", field.name)?;
                writeln!(file, "        unsafe {{ (*self.pointer).base.flags }}")?;
                writeln!(file, "    }}")?;
            }
        }
    }

    writeln!(file, "}}")?;
    writeln!(file)?;

    writeln!(file, "impl std::fmt::Debug for {}<'_> {{", node.name)?;
    writeln!(file, "    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{")?;

    write!(file, "        write!(f, \"{}(", node.name)?;
    if node.fields.is_empty() {
        write!(file, ")\"")?;
    } else {
        let mut padding = false;
        for _ in &node.fields {
            if padding { write!(file, ", ")?; }
            write!(file, "{{:?}}")?;
            padding = true;
        }

        write!(file, ")\", ")?;
        padding = false;

        for field in &node.fields {
            if padding { write!(file, ", ")?; }
            write!(file, "self.{}()", field.name)?;
            padding = true;
        }
    }

    writeln!(file, ")")?;
    writeln!(file, "    }}")?;
    writeln!(file, "}}")?;

    Ok(())
}

/// Write the visit trait to the file.
fn write_visit(file: &mut File, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    writeln!(file, "/// A trait for visiting the AST.")?;
    writeln!(file, "pub trait Visit<'pr> {{")?;
    writeln!(file, "   /// Visits a node.")?;
    writeln!(file, "   fn visit(&mut self, node: &Node<'pr>) {{")?;
    writeln!(file, "       match node {{")?;

    for node in &config.nodes {
        writeln!(file, "           Node::{} {{ parser, pointer, marker }} => self.visit{}(&{} {{ parser: *parser, pointer: *pointer, marker: *marker }}),", node.name, struct_name(&node.name), node.name)?;
    }

    writeln!(file, "       }}")?;
    writeln!(file, "   }}")?;

    for node in &config.nodes {
        writeln!(file)?;
        writeln!(file, "    /// Visits a `{}` node.", node.name)?;
        writeln!(file, "    fn visit{}(&mut self, node: &{}<'pr>) {{", struct_name(&node.name), node.name)?;
        writeln!(file, "        visit{}(self, node);", struct_name(&node.name))?;
        writeln!(file, "    }}")?;
    }
    writeln!(file, "}}")?;

    for node in &config.nodes {
        writeln!(file)?;
        writeln!(file, "/// The default visitor implementation for a `{}` node.", node.name)?;

        let mut children = false;
        for field in &node.fields {
            match field.field_type {
                NodeFieldType::Node | NodeFieldType::OptionalNode | NodeFieldType::NodeList => {
                    children = true;
                    break;
                },
                _ => {}
            }
        }

        if children {
            writeln!(file, "pub fn visit{}<'pr, V>(visitor: &mut V, node: &{}<'pr>)", struct_name(&node.name), node.name)?;
            writeln!(file, "where")?;
            writeln!(file, "    V: Visit<'pr> + ?Sized,")?;
            writeln!(file, "{{")?;

            for field in &node.fields {
                match field.field_type {
                    NodeFieldType::Node => {
                        if let Some(kind) = &field.kind {
                            writeln!(file, "    visitor.visit{}(&node.{}());", struct_name(kind), field.name)?;
                        } else {
                            writeln!(file, "    visitor.visit(&node.{}());", field.name)?;
                        }
                    },
                    NodeFieldType::OptionalNode => {
                        if let Some(kind) = &field.kind {
                            writeln!(file, "    if let Some(node) = node.{}() {{", field.name)?;
                            writeln!(file, "        visitor.visit{}(&node);", struct_name(kind))?;
                            writeln!(file, "    }}")?;
                        } else {
                            writeln!(file, "    if let Some(node) = node.{}() {{", field.name)?;
                            writeln!(file, "        visitor.visit(&node);")?;
                            writeln!(file, "    }}")?;
                        }
                    },
                    NodeFieldType::NodeList => {
                        writeln!(file, "    for node in node.{}().iter() {{", field.name)?;
                        writeln!(file, "        visitor.visit(&node);")?;
                        writeln!(file, "    }}")?;
                    },
                    _ => {}
                }
            }

            writeln!(file, "}}")?;
        } else {
            writeln!(file, "pub fn visit{}<'pr, V>(_visitor: &mut V, _node: &{}<'pr>)", struct_name(&node.name), node.name)?;
            writeln!(file, "where")?;
            writeln!(file, "    V: Visit<'pr> + ?Sized,")?;
            writeln!(file, "{{}}")?;
        }
    }

    Ok(())
}

/// Write the bindings to the `$OUT_DIR/bindings.rs` file. We'll pull these into
/// the actual library in `src/lib.rs`.
fn write_bindings(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let out_path = PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("bindings.rs");
    let mut file = std::fs::File::create(&out_path).expect("Unable to create file");

    writeln!(file, r#"
use std::marker::PhantomData;
use std::ptr::NonNull;

#[allow(clippy::wildcard_imports)]
use yarp_sys::*;

/// A range in the source file.
pub struct Location<'pr> {{
    pointer: NonNull<yp_location_t>,
    marker: PhantomData<&'pr mut yp_location_t>
}}

impl<'pr> Location<'pr> {{
    /// Returns the pointer to the start of the range.
    #[must_use]
    pub fn start(&self) -> *const u8 {{
        unsafe {{ self.pointer.as_ref().start }}
    }}

    /// Returns the pointer to the end of the range.
    #[must_use]
    pub fn end(&self) -> *const u8 {{
        unsafe {{ self.pointer.as_ref().end }}
    }}

    /// Returns a byte slice for the range.
    #[must_use]
    pub fn as_slice(&self) -> &'pr [u8] {{
        let start = self.start();

        unsafe {{
          let len = usize::try_from(self.end().offset_from(start)).expect("end should point to memory after start");
          std::slice::from_raw_parts(start, len)
        }}
    }}
}}

impl std::fmt::Debug for Location<'_> {{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{
        let slice: &[u8] = self.as_slice();

        let mut visible = String::new();
        visible.push('"');

        for &byte in slice {{
            let part: Vec<u8> = std::ascii::escape_default(byte).collect();
            visible.push_str(std::str::from_utf8(&part).unwrap());
        }}

        visible.push('"');
        write!(f, "{{visible}}")
    }}
}}

/// An iterator over the ranges in a list.
pub struct LocationListIter<'pr> {{
    pointer: NonNull<yp_location_list_t>,
    index: usize,
    marker: PhantomData<&'pr mut yp_location_list_t>
}}

impl<'pr> Iterator for LocationListIter<'pr> {{
    type Item = Location<'pr>;

    fn next(&mut self) -> Option<Self::Item> {{
        if self.index >= unsafe {{ self.pointer.as_ref().size }} {{
            None
        }} else {{
            let pointer: *mut yp_location_t = unsafe {{ self.pointer.as_ref().locations.add(self.index) }};
            self.index += 1;
            Some(Location {{ pointer: unsafe {{ NonNull::new_unchecked(pointer) }}, marker: PhantomData }})
        }}
    }}
}}

/// A list of ranges in the source file.
pub struct LocationList<'pr> {{
    pointer: NonNull<yp_location_list_t>,
    marker: PhantomData<&'pr mut yp_location_list_t>
}}

impl<'pr> LocationList<'pr> {{
    /// Returns an iterator over the locations.
    #[must_use]
    pub fn iter(&self) -> LocationListIter<'pr> {{
        LocationListIter {{
            pointer: self.pointer,
            index: 0,
            marker: PhantomData
        }}
    }}
}}

impl std::fmt::Debug for LocationList<'_> {{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{
        write!(f, "{{:?}}", self.iter().collect::<Vec<_>>())
    }}
}}

/// An iterator over the nodes in a list.
pub struct NodeListIter<'pr> {{
    parser: NonNull<yp_parser_t>,
    pointer: NonNull<yp_node_list>,
    index: usize,
    marker: PhantomData<&'pr mut yp_node_list>
}}

impl<'pr> Iterator for NodeListIter<'pr> {{
    type Item = Node<'pr>;

    fn next(&mut self) -> Option<Self::Item> {{
        if self.index >= unsafe {{ self.pointer.as_ref().size }} {{
            None
        }} else {{
            let node: *mut yp_node_t = unsafe {{ *(self.pointer.as_ref().nodes.add(self.index)) }};
            self.index += 1;
            Some(Node::new(self.parser, node))
        }}
    }}
}}

/// A list of nodes.
pub struct NodeList<'pr> {{
    parser: NonNull<yp_parser_t>,
    pointer: NonNull<yp_node_list>,
    marker: PhantomData<&'pr mut yp_node_list>
}}

impl<'pr> NodeList<'pr> {{
    /// Returns an iterator over the nodes.
    #[must_use]
    pub fn iter(&self) -> NodeListIter<'pr> {{
        NodeListIter {{
            parser: self.parser,
            pointer: self.pointer,
            index: 0,
            marker: PhantomData
        }}
    }}
}}

impl std::fmt::Debug for NodeList<'_> {{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{
        write!(f, "{{:?}}", self.iter().collect::<Vec<_>>())
    }}
}}

/// A handle for a constant ID.
pub struct ConstantId<'pr> {{
    parser: NonNull<yp_parser_t>,
    id: yp_constant_id_t,
    marker: PhantomData<&'pr mut yp_constant_id_t>
}}

impl<'pr> ConstantId<'pr> {{
    fn new(parser: NonNull<yp_parser_t>, id: yp_constant_id_t) -> Self {{
        ConstantId {{ parser, id, marker: PhantomData }}
    }}

    /// Returns a byte slice for the constant ID.
    #[must_use]
    pub fn as_slice(&self) -> &'pr [u8] {{
        unsafe {{
            let pool = &(*self.parser.as_ptr()).constant_pool;
            let constant = &(*pool.constants.offset(isize::try_from(self.id).expect("id should be in range")));
            std::slice::from_raw_parts(constant.start, constant.length)
        }}
    }}
}}

impl std::fmt::Debug for ConstantId<'_> {{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{
        write!(f, "{{:?}}", self.id)
    }}
}}

/// An iterator over the constants in a list.
pub struct ConstantListIter<'pr> {{
    parser: NonNull<yp_parser_t>,
    pointer: NonNull<yp_constant_id_list_t>,
    index: usize,
    marker: PhantomData<&'pr mut yp_constant_id_list_t>
}}

impl<'pr> Iterator for ConstantListIter<'pr> {{
    type Item = ConstantId<'pr>;

    fn next(&mut self) -> Option<Self::Item> {{
        if self.index >= unsafe {{ self.pointer.as_ref().size }} {{
            None
        }} else {{
            let constant_id: yp_constant_id_t = unsafe {{ *(self.pointer.as_ref().ids.add(self.index)) }};
            self.index += 1;
            Some(ConstantId::new(self.parser, constant_id))
        }}
    }}
}}

/// A list of constants.
pub struct ConstantList<'pr> {{
    /// The raw pointer to the parser where this list came from.
    parser: NonNull<yp_parser_t>,

    /// The raw pointer to the list allocated by YARP.
    pointer: NonNull<yp_constant_id_list_t>,

    /// The marker to indicate the lifetime of the pointer.
    marker: PhantomData<&'pr mut yp_constant_id_list_t>
}}

impl<'pr> ConstantList<'pr> {{
    /// Returns an iterator over the constants in the list.
    #[must_use]
    pub fn iter(&self) -> ConstantListIter<'pr> {{
        ConstantListIter {{
            parser: self.parser,
            pointer: self.pointer,
            index: 0,
            marker: PhantomData
        }}
    }}
}}

impl std::fmt::Debug for ConstantList<'_> {{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{
        write!(f, "{{:?}}", self.iter().collect::<Vec<_>>())
    }}
}}
"#)?;

    for node in &config.nodes {
        writeln!(file, "const {}: u16 = yp_node_type::{} as u16;", type_name(&node.name), type_name(&node.name))?;
    }
    writeln!(file)?;

    writeln!(file, "/// An enum representing the different kinds of nodes that can be parsed.")?;
    writeln!(file, "pub enum Node<'pr> {{")?;

    for node in &config.nodes {
        writeln!(file, "    /// The {} node", node.name)?;
        writeln!(file, "    {} {{", node.name)?;
        writeln!(file, "        /// The pointer to the associated parser this node came from.")?;
        writeln!(file, "        parser: NonNull<yp_parser_t>,")?;
        writeln!(file)?;
        writeln!(file, "        /// The raw pointer to the node allocated by YARP.")?;
        writeln!(file, "        pointer: *mut yp{}_t,", struct_name(&node.name))?;
        writeln!(file)?;
        writeln!(file, "        /// The marker to indicate the lifetime of the pointer.")?;
        writeln!(file, "        marker: PhantomData<&'pr mut yp{}_t>", struct_name(&node.name))?;
        writeln!(file, "    }},")?;
    }

    writeln!(file, "}}")?;
    writeln!(file)?;

    writeln!(file, r#"
impl<'pr> Node<'pr> {{
    /// Creates a new node from the given pointer.
    ///
    /// # Panics
    ///
    /// Panics if the node type cannot be read.
    ///
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub(crate) fn new(parser: NonNull<yp_parser_t>, node: *mut yp_node_t) -> Self {{
        match unsafe {{ (*node).type_ }} {{
"#)?;

    for node in &config.nodes {
        writeln!(file, "            {} => Self::{} {{ parser, pointer: node.cast::<yp{}_t>(), marker: PhantomData }},", type_name(&node.name), node.name, struct_name(&node.name))?;
    }

    writeln!(file, "            _ => panic!(\"Unknown node type: {{}}\", unsafe {{ (*node).type_ }})")?;
    writeln!(file, "        }}")?;
    writeln!(file, "    }}")?;
    writeln!(file)?;

    writeln!(file, "    /// Returns the location of this node.")?;
    writeln!(file, "    #[must_use]")?;
    writeln!(file, "    pub fn location(&self) -> Location<'pr> {{")?;
    writeln!(file, "        match *self {{")?;
    for node in &config.nodes {
        writeln!(file, "            Self::{} {{ pointer, .. }} => Location {{ pointer: unsafe {{ NonNull::new_unchecked(&mut (*pointer.cast::<yp_node_t>()).location) }}, marker: PhantomData }},", node.name)?;
    }
    writeln!(file, "        }}")?;
    writeln!(file, "    }}")?;
    writeln!(file)?;

    for node in &config.nodes {
        writeln!(file, "    /// Returns the node as a `{}`.", node.name)?;
        writeln!(file, "    #[must_use]")?;
        writeln!(file, "    pub fn as{}(&self) -> Option<{}<'_>> {{", struct_name(&node.name), node.name)?;
        writeln!(file, "        match *self {{")?;
        writeln!(file, "            Self::{} {{ parser, pointer, marker }} => Some({} {{ parser, pointer, marker }}),", node.name, node.name)?;
        writeln!(file, "            _ => None")?;
        writeln!(file, "        }}")?;
        writeln!(file, "    }}")?;
    }

    writeln!(file, "}}")?;
    writeln!(file)?;

    writeln!(file, "impl std::fmt::Debug for Node<'_> {{")?;
    writeln!(file, "    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{")?;
    writeln!(file, "        match *self {{")?;

    for node in &config.nodes {
        writeln!(file, "            Self::{} {{ parser, pointer, marker }} => write!(f, \"{{:?}}\", {} {{ parser, pointer, marker }}),", node.name, node.name)?;
    }

    writeln!(file, "        }}")?;
    writeln!(file, "    }}")?;
    writeln!(file, "}}")?;
    writeln!(file)?;

    for node in &config.nodes {
        write_node(&mut file, node)?;
        writeln!(file)?;
    }

    write_visit(&mut file, config)?;

    Ok(())
}
