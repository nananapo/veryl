use crate::analyze_error::AnalyzeError;
use crate::symbol_table::{Name, Namespace, SymbolKind, SymbolTable};
use veryl_parser::global_table;
use veryl_parser::miette::Result;
use veryl_parser::veryl_grammar_trait::*;
use veryl_parser::veryl_walker::{Handler, HandlerPoint};

pub struct CheckModuleInstance<'a> {
    pub errors: Vec<AnalyzeError>,
    text: &'a str,
    symbol_table: &'a SymbolTable,
    point: HandlerPoint,
    namespace: Namespace,
}

impl<'a> CheckModuleInstance<'a> {
    pub fn new(text: &'a str, symbol_table: &'a SymbolTable) -> Self {
        Self {
            errors: Vec::new(),
            text,
            symbol_table,
            point: HandlerPoint::Before,
            namespace: Namespace::default(),
        }
    }
}

impl<'a> Handler for CheckModuleInstance<'a> {
    fn set_point(&mut self, p: HandlerPoint) {
        self.point = p;
    }
}

impl<'a> VerylGrammarTrait for CheckModuleInstance<'a> {
    fn inst_declaration(&mut self, arg: &InstDeclaration) -> Result<()> {
        if let HandlerPoint::Before = self.point {
            let mut name = Vec::new();
            name.push(arg.scoped_identifier.identifier.identifier_token.token.text);
            for x in &arg.scoped_identifier.scoped_identifier_list {
                name.push(x.identifier.identifier_token.token.text);
            }
            let name = &Name::Hierarchical(name);

            let mut connected_ports = Vec::new();
            if let Some(ref x) = arg.inst_declaration_opt1 {
                if let Some(ref x) = x.inst_declaration_opt2 {
                    let x = &x.inst_port_list;
                    connected_ports.push(x.inst_port_item.identifier.identifier_token.token.text);
                    for x in &x.inst_port_list_list {
                        connected_ports
                            .push(x.inst_port_item.identifier.identifier_token.token.text);
                    }
                }
            }

            let symbol = self.symbol_table.get(name, &self.namespace);
            if let Some(symbol) = symbol {
                if let SymbolKind::Module { ref ports, .. } = symbol.kind {
                    for port in ports {
                        if !connected_ports.contains(&port.name) {
                            let name =
                                global_table::get_str_value(*name.as_slice().last().unwrap())
                                    .unwrap();
                            let port = global_table::get_str_value(port.name).unwrap();
                            self.errors.push(AnalyzeError::missing_port(
                                &name,
                                &port,
                                self.text,
                                &arg.identifier.identifier_token,
                            ));
                        }
                    }
                    for port in &connected_ports {
                        if !ports.iter().any(|x| &x.name == port) {
                            let name =
                                global_table::get_str_value(*name.as_slice().last().unwrap())
                                    .unwrap();
                            let port = global_table::get_str_value(*port).unwrap();
                            self.errors.push(AnalyzeError::unknown_port(
                                &name,
                                &port,
                                self.text,
                                &arg.identifier.identifier_token,
                            ));
                        }
                    }
                } else {
                    let name =
                        global_table::get_str_value(*name.as_slice().last().unwrap()).unwrap();
                    self.errors.push(AnalyzeError::mismatch_type(
                        &name,
                        "module",
                        &symbol.kind.to_string(),
                        self.text,
                        &arg.identifier.identifier_token,
                    ));
                }
            }
        }
        Ok(())
    }

    fn function_declaration(&mut self, arg: &FunctionDeclaration) -> Result<()> {
        match self.point {
            HandlerPoint::Before => {
                let name = arg.identifier.identifier_token.token.text;
                self.namespace.push(name)
            }
            HandlerPoint::After => self.namespace.pop(),
        }
        Ok(())
    }

    fn module_declaration(&mut self, arg: &ModuleDeclaration) -> Result<()> {
        match self.point {
            HandlerPoint::Before => {
                let name = arg.identifier.identifier_token.token.text;
                self.namespace.push(name)
            }
            HandlerPoint::After => self.namespace.pop(),
        }
        Ok(())
    }

    fn interface_declaration(&mut self, arg: &InterfaceDeclaration) -> Result<()> {
        match self.point {
            HandlerPoint::Before => {
                let name = arg.identifier.identifier_token.token.text;
                self.namespace.push(name)
            }
            HandlerPoint::After => self.namespace.pop(),
        }
        Ok(())
    }
}