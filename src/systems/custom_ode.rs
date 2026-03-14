/// Custom ODE system: user-defined expressions for dx/dt, dy/dt, dz/dt.
/// Uses a simple recursive-descent evaluator.

use super::{DynamicalSystem, rk4};

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    LParen,
    RParen,
    End,
}

fn tokenize(src: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            ' ' | '\t' | '\r' | '\n' => { i += 1; }
            '+' => { tokens.push(Token::Plus);   i += 1; }
            '-' => { tokens.push(Token::Minus);  i += 1; }
            '*' => { tokens.push(Token::Star);   i += 1; }
            '/' => { tokens.push(Token::Slash);  i += 1; }
            '^' => { tokens.push(Token::Caret);  i += 1; }
            '(' => { tokens.push(Token::LParen); i += 1; }
            ')' => { tokens.push(Token::RParen); i += 1; }
            c if c.is_ascii_digit() || c == '.' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == 'e' || chars[i] == 'E') {
                    // handle scientific notation sign
                    if (chars[i] == 'e' || chars[i] == 'E') && i + 1 < chars.len() && (chars[i+1] == '+' || chars[i+1] == '-') {
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                let s: String = chars[start..i].iter().collect();
                let v = s.parse::<f64>().unwrap_or(0.0);
                tokens.push(Token::Number(v));
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let name: String = chars[start..i].iter().collect();
                tokens.push(Token::Ident(name));
            }
            _ => { i += 1; } // skip unknown characters
        }
    }
    tokens.push(Token::End);
    tokens
}

// ---------------------------------------------------------------------------
// Recursive descent parser/evaluator
// ---------------------------------------------------------------------------

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    x: f64,
    y: f64,
    z: f64,
    t: f64,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token], x: f64, y: f64, z: f64, t: f64) -> Self {
        Self { tokens, pos: 0, x, y, z, t }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn consume(&mut self) -> &Token {
        let t = &self.tokens[self.pos.min(self.tokens.len() - 1)];
        if self.pos < self.tokens.len() - 1 { self.pos += 1; }
        t
    }

    /// expression = term (('+' | '-') term)*
    fn expression(&mut self) -> f64 {
        let mut val = self.term();
        loop {
            match self.peek() {
                Token::Plus  => { self.consume(); val += self.term(); }
                Token::Minus => { self.consume(); val -= self.term(); }
                _ => break,
            }
        }
        val
    }

    /// term = power (('*' | '/') power)*
    fn term(&mut self) -> f64 {
        let mut val = self.power();
        loop {
            match self.peek() {
                Token::Star  => { self.consume(); let r = self.power(); val *= r; }
                Token::Slash => {
                    self.consume();
                    let r = self.power();
                    val = if r.abs() > 1e-300 { val / r } else { 0.0 };
                }
                _ => break,
            }
        }
        val
    }

    /// power = unary ('^' unary)?
    fn power(&mut self) -> f64 {
        let base = self.unary();
        if let Token::Caret = self.peek() {
            self.consume();
            let exp = self.unary();
            base.powf(exp)
        } else {
            base
        }
    }

    /// unary = '-' unary | primary
    fn unary(&mut self) -> f64 {
        if let Token::Minus = self.peek() {
            self.consume();
            -self.unary()
        } else {
            self.primary()
        }
    }

    /// primary = number | variable | function '(' expr ')' | '(' expr ')'
    fn primary(&mut self) -> f64 {
        match self.peek().clone() {
            Token::Number(v) => { self.consume(); v }
            Token::LParen => {
                self.consume();
                let v = self.expression();
                if let Token::RParen = self.peek() { self.consume(); }
                v
            }
            Token::Ident(ref name) => {
                let name = name.clone();
                self.consume();
                // Check if it's a function call
                if let Token::LParen = self.peek() {
                    self.consume();
                    let arg = self.expression();
                    if let Token::RParen = self.peek() { self.consume(); }
                    match name.as_str() {
                        "sin"  => arg.sin(),
                        "cos"  => arg.cos(),
                        "exp"  => arg.exp(),
                        "abs"  => arg.abs(),
                        "sqrt" => if arg >= 0.0 { arg.sqrt() } else { 0.0 },
                        "ln"   => if arg > 0.0 { arg.ln() } else { 0.0 },
                        "log"  => if arg > 0.0 { arg.log10() } else { 0.0 },
                        "tan"  => arg.tan(),
                        _ => 0.0,
                    }
                } else {
                    // Variable
                    match name.as_str() {
                        "x" => self.x,
                        "y" => self.y,
                        "z" => self.z,
                        "t" => self.t,
                        "pi" | "PI" => std::f64::consts::PI,
                        "e"  | "E"  => std::f64::consts::E,
                        _ => 0.0,
                    }
                }
            }
            _ => { self.consume(); 0.0 }
        }
    }
}

/// Evaluate an expression string for given x, y, z, t variables.
/// Returns 0.0 if the expression fails to parse or produces non-finite output.
pub fn eval_expr(src: &str, x: f64, y: f64, z: f64, t: f64) -> f64 {
    let tokens = tokenize(src);
    if tokens.is_empty() { return 0.0; }
    let mut parser = Parser::new(&tokens, x, y, z, t);
    let val = parser.expression();
    if val.is_finite() { val } else { 0.0 }
}

// ---------------------------------------------------------------------------
// CustomOde DynamicalSystem
// ---------------------------------------------------------------------------

pub struct CustomOde {
    pub expr_x: String,
    pub expr_y: String,
    pub expr_z: String,
    state: Vec<f64>,
    t: f64,
    last_speed: f64,
}

impl CustomOde {
    pub fn new(expr_x: String, expr_y: String, expr_z: String) -> Self {
        Self {
            expr_x,
            expr_y,
            expr_z,
            state: vec![1.0, 0.0, 0.0], // start further from origin for faster attractor development
            t: 0.0,
            last_speed: 0.0,
        }
    }

    fn deriv_internal(&self, state: &[f64]) -> Vec<f64> {
        let (x, y, z) = (state[0], state[1], state[2]);
        let dx = eval_expr(&self.expr_x, x, y, z, self.t).clamp(-1e6, 1e6);
        let dy = eval_expr(&self.expr_y, x, y, z, self.t).clamp(-1e6, 1e6);
        let dz = eval_expr(&self.expr_z, x, y, z, self.t).clamp(-1e6, 1e6);
        vec![dx, dy, dz]
    }
}

impl DynamicalSystem for CustomOde {
    fn state(&self) -> &[f64] { &self.state }

    fn step(&mut self, dt: f64) {
        let deriv_before = self.deriv_internal(&self.state);
        let ex = &self.expr_x;
        let ey = &self.expr_y;
        let ez = &self.expr_z;
        let t = self.t;
        rk4(&mut self.state, dt, |s| {
            let (x, y, z) = (s[0], s[1], s[2]);
            vec![
                eval_expr(ex, x, y, z, t).clamp(-1e6, 1e6),
                eval_expr(ey, x, y, z, t).clamp(-1e6, 1e6),
                eval_expr(ez, x, y, z, t).clamp(-1e6, 1e6),
            ]
        });
        self.t += dt;

        // Reset if state blows up (divide-by-zero, unstable ODE, etc.)
        let magnitude = self.state[0].powi(2) + self.state[1].powi(2) + self.state[2].powi(2);
        if magnitude > 1e10 || !self.state[0].is_finite() || !self.state[1].is_finite() || !self.state[2].is_finite() {
            self.state = vec![1.0, 0.0, 0.0];
        }

        let speed = (deriv_before[0].powi(2) + deriv_before[1].powi(2) + deriv_before[2].powi(2)).sqrt();
        self.last_speed = speed;
    }

    fn dimension(&self) -> usize { 3 }
    fn name(&self) -> &str { "custom" }

    fn speed(&self) -> f64 { self.last_speed }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        self.deriv_internal(state)
    }
}

/// Try to validate expressions by evaluating at a test point.
/// Returns Ok(()) if all three produce finite results, Err with message otherwise.
pub fn validate_exprs(ex: &str, ey: &str, ez: &str) -> Result<(), String> {
    let dx = eval_expr(ex, 1.0, 1.0, 1.0, 0.0);
    let dy = eval_expr(ey, 1.0, 1.0, 1.0, 0.0);
    let dz = eval_expr(ez, 1.0, 1.0, 1.0, 0.0);
    if !dx.is_finite() { return Err(format!("dx/dt expression error at test point")); }
    if !dy.is_finite() { return Err(format!("dy/dt expression error at test point")); }
    if !dz.is_finite() { return Err(format!("dz/dt expression error at test point")); }
    Ok(())
}
