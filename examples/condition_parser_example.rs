// This example demonstrates how to use the condition parser
// Run with: cargo run --example condition_parser_example

// Include the modules directly since this is an example
#[path = "../src/condition_variables.rs"]
mod condition_variables;

#[path = "../src/condition_parser.rs"]
mod condition_parser;

use condition_parser::{parse_condition, CompList};

fn main() {
    println!("=== Skill Activation Condition Parser Example ===");
    println!("Note: '&' (AND) has higher precedence than '@' (OR)\n");

    // Example 1: Simple condition
    let example1 = "hp == 100";
    println!("Input: {}", example1);
    match parse_condition(example1) {
        Ok(condition) => {
            println!("Parsed: {}", condition);
            println!("AST: {:#?}\n", condition);
        }
        Err(e) => println!("Error: {}\n", e),
    }

    // Example 2: Binary AND condition
    let example2 = "hp > 50 & stamina >= 30";
    println!("Input: {}", example2);
    println!("Meaning: hp > 50 AND stamina >= 30 (both must be true)");
    match parse_condition(example2) {
        Ok(condition) => {
            println!("Parsed: {}", condition);
            println!("AST: {:#?}\n", condition);
        }
        Err(e) => println!("Error: {}\n", e),
    }

    // Example 3: OR operator
    let example3 = "position <= 3 @ phase == 2";
    println!("Input: {}", example3);
    println!("Meaning: position <= 3 OR phase == 2 (at least one must be true)");
    match parse_condition(example3) {
        Ok(condition) => {
            println!("Parsed: {}", condition);
            println!("AST: {:#?}\n", condition);
        }
        Err(e) => println!("Error: {}\n", e),
    }

    // Example 4: Precedence demonstration
    let example4 = "a == 1 & b == 2 @ c == 3 & d == 4";
    println!("Input: {}", example4);
    println!("Parsed as: (a == 1 & b == 2) @ (c == 3 & d == 4)");
    println!("Meaning: (a AND b) OR (c AND d)");
    match parse_condition(example4) {
        Ok(condition) => {
            println!("Display: {}", condition);
            println!("AST: {:#?}\n", condition);
        }
        Err(e) => println!("Error: {}\n", e),
    }

    // Example 5: Complex nested condition
    let example5 = "position <= 3 & distance_remaining > 500 @ phase == 2";
    println!("Input: {}", example5);
    println!("Parsed as: (position <= 3 & distance_remaining > 500) @ phase == 2");
    println!("Meaning: skill activates if:");
    println!("  - (in top 3 AND more than 500m remaining) OR (in phase 2)");
    match parse_condition(example5) {
        Ok(condition) => {
            println!("Display: {}", condition);
            println!("AST: {:#?}\n", condition);
        }
        Err(e) => println!("Error: {}\n", e),
    }

    // Example 6: Left associativity
    let example6 = "a == 1 @ b == 2 @ c == 3";
    println!("Input: {}", example6);
    println!("Parsed as: (a == 1 @ b == 2) @ c == 3 (left-associative)");
    println!("Meaning: a OR b OR c (any one can be true)");
    match parse_condition(example6) {
        Ok(condition) => {
            println!("Display: {}", condition);
            println!("AST: {:#?}\n", condition);
        }
        Err(e) => println!("Error: {}\n", e),
    }

    // Example 7: Error case
    println!("\n=== Error Handling ===");
    let invalid = "hp = 100"; // Single = instead of ==
    println!("Input: {}", invalid);
    match parse_condition(invalid) {
        Ok(condition) => println!("Parsed: {}", condition),
        Err(e) => println!("Error (expected): {}\n", e),
    }

    // Example 8: Demonstrating pattern matching on the AST
    println!("=== Pattern Matching Example ===");
    let condition_str = "running_style == 1 & stamina > 100";
    println!("Input: {}", condition_str);

    if let Ok(condition) = parse_condition(condition_str) {
        match &condition.comp_list {
            CompList::Single(comp) => {
                println!("Single comparison:");
                println!("  Variable: {}", comp.var);
                println!("  Operator: {}", comp.op);
                println!("  Value: {}", comp.value);
            }
            CompList::Binary { left, op, right } => {
                println!("Binary expression:");
                println!("  Operator: {}", op);
                print!("  Left: ");
                print_comp_list(left, 2);
                print!("  Right: ");
                print_comp_list(right, 2);
            }
        }
    }
}

fn print_comp_list(comp_list: &CompList, _indent: usize) {
    match comp_list {
        CompList::Single(comp) => {
            println!("{} {} {}", comp.var, comp.op, comp.value);
        }
        CompList::Binary { left, op, right } => {
            print!("(");
            print_comp_list_inline(left);
            print!(" {} ", op);
            print_comp_list_inline(right);
            println!(")");
        }
    }
}

fn print_comp_list_inline(comp_list: &CompList) {
    match comp_list {
        CompList::Single(comp) => {
            print!("{} {} {}", comp.var, comp.op, comp.value);
        }
        CompList::Binary { left, op, right } => {
            print!("(");
            print_comp_list_inline(left);
            print!(" {} ", op);
            print_comp_list_inline(right);
            print!(")");
        }
    }
}
