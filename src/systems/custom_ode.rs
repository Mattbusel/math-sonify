//! Custom ODE system: user-defined expressions for dx/dt, dy/dt, dz/dt.
//! Uses a simple recursive-descent evaluator.

use super::{rk4, DynamicalSystem};

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
            ' ' | '\t' | '\r' | '\n' => {
                i += 1;
            }
            '+' => {
                tokens.push(Token::Plus);
                i += 1;
            }
            '-' => {
                tokens.push(Token::Minus);
                i += 1;
            }
            '*' => {
                tokens.push(Token::Star);
                i += 1;
            }
            '/' => {
                tokens.push(Token::Slash);
                i += 1;
            }
            '^' => {
                tokens.push(Token::Caret);
                i += 1;
            }
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            c if c.is_ascii_digit() || c == '.' => {
                let start = i;
                while i < chars.len()
                    && (chars[i].is_ascii_digit()
                        || chars[i] == '.'
                        || chars[i] == 'e'
                        || chars[i] == 'E')
                {
                    // handle scientific notation sign
                    if (chars[i] == 'e' || chars[i] == 'E')
                        && i + 1 < chars.len()
                        && (chars[i + 1] == '+' || chars[i + 1] == '-')
                    {
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
            _ => {
                i += 1;
            } // skip unknown characters
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
    w: f64,
    t: f64,
    /// When `true`, division by zero propagates `f64::NAN` rather than 0.0.
    /// Used by `eval_expr_4d_raw` so that `validate_exprs` can detect it.
    strict: bool,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token], x: f64, y: f64, z: f64, w: f64, t: f64) -> Self {
        Self {
            tokens,
            pos: 0,
            x,
            y,
            z,
            w,
            t,
            strict: false,
        }
    }

    fn new_strict(tokens: &'a [Token], x: f64, y: f64, z: f64, w: f64, t: f64) -> Self {
        Self { strict: true, ..Self::new(tokens, x, y, z, w, t) }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn consume(&mut self) -> &Token {
        let t = &self.tokens[self.pos.min(self.tokens.len() - 1)];
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        t
    }

    /// expression = term (('+' | '-') term)*
    fn expression(&mut self) -> f64 {
        let mut val = self.term();
        loop {
            match self.peek() {
                Token::Plus => {
                    self.consume();
                    val += self.term();
                }
                Token::Minus => {
                    self.consume();
                    val -= self.term();
                }
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
                Token::Star => {
                    self.consume();
                    let r = self.power();
                    val *= r;
                }
                Token::Slash => {
                    self.consume();
                    let r = self.power();
                    val = if self.strict {
                        val / r  // propagate inf/NaN for validate_exprs detection
                    } else if r.abs() > 1e-300 {
                        val / r
                    } else {
                        0.0  // safe coercion during integration
                    };
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
            Token::Number(v) => {
                self.consume();
                v
            }
            Token::LParen => {
                self.consume();
                let v = self.expression();
                if let Token::RParen = self.peek() {
                    self.consume();
                }
                v
            }
            Token::Ident(ref name) => {
                let name = name.clone();
                self.consume();
                // Check if it's a function call
                if let Token::LParen = self.peek() {
                    self.consume();
                    let arg = self.expression();
                    if let Token::RParen = self.peek() {
                        self.consume();
                    }
                    match name.as_str() {
                        "sin" => arg.sin(),
                        "cos" => arg.cos(),
                        "exp" => arg.exp(),
                        "abs" => arg.abs(),
                        "sqrt" => {
                            if arg >= 0.0 {
                                arg.sqrt()
                            } else {
                                0.0
                            }
                        }
                        "ln" => {
                            if arg > 0.0 {
                                arg.ln()
                            } else {
                                0.0
                            }
                        }
                        "log" => {
                            if arg > 0.0 {
                                arg.log10()
                            } else {
                                0.0
                            }
                        }
                        "tan" => arg.tan(),
                        _ => 0.0,
                    }
                } else {
                    // Variable
                    match name.as_str() {
                        "x" => self.x,
                        "y" => self.y,
                        "z" => self.z,
                        "w" => self.w,
                        "t" => self.t,
                        "pi" | "PI" => std::f64::consts::PI,
                        "e" | "E" => std::f64::consts::E,
                        _ => 0.0,
                    }
                }
            }
            _ => {
                self.consume();
                0.0
            }
        }
    }
}

/// Evaluate an expression string for given x, y, z, w, t variables.
/// Returns 0.0 if the expression fails to parse or produces non-finite output.
pub fn eval_expr(src: &str, x: f64, y: f64, z: f64, t: f64) -> f64 {
    eval_expr_4d(src, x, y, z, 0.0, t)
}

/// Evaluate an expression string with all four variables x, y, z, w and time t.
pub fn eval_expr_4d(src: &str, x: f64, y: f64, z: f64, w: f64, t: f64) -> f64 {
    let tokens = tokenize(src);
    if tokens.is_empty() {
        return 0.0;
    }
    let mut parser = Parser::new(&tokens, x, y, z, w, t);
    let val = parser.expression();
    if val.is_finite() { val } else { 0.0 }
}

/// Like `eval_expr_4d` but uses strict division (propagates NaN/inf for
/// division-by-zero rather than coercing to 0.0).  Used by `validate_exprs`
/// so that problematic sub-expressions are detectable.
fn eval_expr_4d_raw(src: &str, x: f64, y: f64, z: f64, w: f64, t: f64) -> f64 {
    let tokens = tokenize(src);
    if tokens.is_empty() {
        return 0.0;
    }
    let mut parser = Parser::new_strict(&tokens, x, y, z, w, t);
    parser.expression()
}

// ---------------------------------------------------------------------------
// CustomOde DynamicalSystem
// ---------------------------------------------------------------------------

pub struct CustomOde {
    pub expr_x: String,
    pub expr_y: String,
    pub expr_z: String,
    /// Optional 4th equation (dw/dt). Empty string = 3-variable mode.
    pub expr_w: String,
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
            expr_w: String::new(),
            state: vec![1.0, 0.0, 0.0, 0.0],
            t: 0.0,
            last_speed: 0.0,
        }
    }

    /// Returns true when a 4th equation (dw/dt) is active.
    pub fn is_4d(&self) -> bool {
        !self.expr_w.trim().is_empty()
    }

    fn deriv_internal(&self, state: &[f64]) -> Vec<f64> {
        let x = state[0];
        let y = state[1];
        let z = state[2];
        let w = state.get(3).copied().unwrap_or(0.0);
        let dx = eval_expr_4d(&self.expr_x, x, y, z, w, self.t).clamp(-1e6, 1e6);
        let dy = eval_expr_4d(&self.expr_y, x, y, z, w, self.t).clamp(-1e6, 1e6);
        let dz = eval_expr_4d(&self.expr_z, x, y, z, w, self.t).clamp(-1e6, 1e6);
        if self.is_4d() {
            let dw = eval_expr_4d(&self.expr_w, x, y, z, w, self.t).clamp(-1e6, 1e6);
            vec![dx, dy, dz, dw]
        } else {
            vec![dx, dy, dz, 0.0]
        }
    }
}

impl DynamicalSystem for CustomOde {
    fn state(&self) -> &[f64] {
        &self.state
    }

    fn step(&mut self, dt: f64) {
        let deriv_before = self.deriv_internal(&self.state);
        let ex = self.expr_x.clone();
        let ey = self.expr_y.clone();
        let ez = self.expr_z.clone();
        let ew = self.expr_w.clone();
        let is_4d = self.is_4d();
        let t = self.t;
        rk4(&mut self.state, dt, |s| {
            let x = s[0]; let y = s[1]; let z = s[2];
            let w = s.get(3).copied().unwrap_or(0.0);
            let dw = if is_4d {
                eval_expr_4d(&ew, x, y, z, w, t).clamp(-1e6, 1e6)
            } else {
                0.0
            };
            vec![
                eval_expr_4d(&ex, x, y, z, w, t).clamp(-1e6, 1e6),
                eval_expr_4d(&ey, x, y, z, w, t).clamp(-1e6, 1e6),
                eval_expr_4d(&ez, x, y, z, w, t).clamp(-1e6, 1e6),
                dw,
            ]
        });
        self.t += dt;

        // Reset if state blows up (divide-by-zero, unstable ODE, etc.)
        let magnitude = self.state[0].powi(2) + self.state[1].powi(2)
            + self.state[2].powi(2) + self.state.get(3).copied().unwrap_or(0.0).powi(2);
        if magnitude > 1e10 || !self.state.iter().all(|v| v.is_finite()) {
            self.state = vec![1.0, 0.0, 0.0, 0.0];
        }

        let speed = (deriv_before[0].powi(2) + deriv_before[1].powi(2)
            + deriv_before[2].powi(2)).sqrt();
        self.last_speed = speed;
    }

    fn dimension(&self) -> usize {
        if self.is_4d() { 4 } else { 3 }
    }
    fn name(&self) -> &str {
        "custom"
    }

    fn speed(&self) -> f64 {
        self.last_speed
    }

    fn deriv_at(&self, state: &[f64]) -> Vec<f64> {
        self.deriv_internal(state)
    }
}

/// Scan an expression string for identifier tokens that are not known variables or functions.
/// Returns a warning string if suspicious identifiers are found (likely typos).
fn warn_unknown_idents(src: &str) -> Option<String> {
    const KNOWN: &[&str] = &[
        "x", "y", "z", "w", "t", "pi", "PI", "e", "E", "sin", "cos", "exp", "abs", "sqrt", "ln",
        "log", "tan",
    ];
    let tokens = tokenize(src);
    let mut unknowns: Vec<String> = Vec::new();
    for (i, tok) in tokens.iter().enumerate() {
        if let Token::Ident(name) = tok {
            if !KNOWN.contains(&name.as_str()) {
                // Only flag if NOT followed by '(' (which would be an unknown function)
                let next = tokens.get(i + 1);
                let is_fn_call = matches!(next, Some(Token::LParen));
                if is_fn_call || !unknowns.contains(name) {
                    unknowns.push(name.clone());
                }
            }
        }
    }
    if unknowns.is_empty() {
        None
    } else {
        Some(format!("unknown identifier(s): {}", unknowns.join(", ")))
    }
}

/// Try to validate expressions by evaluating at multiple test points.
/// Pass `ew = ""` for 3-variable mode; a non-empty `ew` enables 4D validation.
/// Returns Ok(()) if all equations produce finite results at all test points.
/// Returns Err with a descriptive message if an issue is detected, including
/// warnings about unknown identifiers (likely typos).
#[allow(dead_code)]
pub fn validate_exprs(ex: &str, ey: &str, ez: &str, ew: &str) -> Result<(), String> {
    let test_points: &[(f64, f64, f64, f64, f64)] = &[
        (1.0, 1.0, 1.0, 0.5, 0.0),
        (0.0, 0.0, 0.0, 0.0, 0.0),
        (-1.0, -1.0, -1.0, -0.5, 0.0),
        (5.0, -3.0, 2.0, 1.0, 1.0),
    ];
    for &(x, y, z, w, t) in test_points {
        // Use the raw evaluator so that division-by-zero (→ ±inf) and overflow
        // are not silently coerced to 0.0 before the finite check.
        let dx = eval_expr_4d_raw(ex, x, y, z, w, t);
        let dy = eval_expr_4d_raw(ey, x, y, z, w, t);
        let dz = eval_expr_4d_raw(ez, x, y, z, w, t);
        if !dx.is_finite() {
            return Err(format!("dx/dt error at ({x},{y},{z}): result is {dx}"));
        }
        if !dy.is_finite() {
            return Err(format!("dy/dt error at ({x},{y},{z}): result is {dy}"));
        }
        if !dz.is_finite() {
            return Err(format!("dz/dt error at ({x},{y},{z}): result is {dz}"));
        }
        if !ew.trim().is_empty() {
            let dw = eval_expr_4d_raw(ew, x, y, z, w, t);
            if !dw.is_finite() {
                return Err(format!("dw/dt error at ({x},{y},{z},{w}): result is {dw}"));
            }
        }
    }
    // Warn about unknown identifiers (typo detection)
    let mut warnings = Vec::new();
    for (label, expr) in [("dx/dt", ex), ("dy/dt", ey), ("dz/dt", ez), ("dw/dt", ew)] {
        if expr.trim().is_empty() { continue; }
        if let Some(w) = warn_unknown_idents(expr) {
            warnings.push(format!("{label}: {w}"));
        }
    }
    if !warnings.is_empty() {
        return Err(format!("Warning — {}", warnings.join("; ")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::DynamicalSystem;

    #[test]
    fn test_eval_expr_basic_arithmetic() {
        assert!((eval_expr("2 + 3", 0.0, 0.0, 0.0, 0.0) - 5.0).abs() < 1e-10);
        assert!((eval_expr("4 * 2", 0.0, 0.0, 0.0, 0.0) - 8.0).abs() < 1e-10);
        assert!((eval_expr("10 / 2", 0.0, 0.0, 0.0, 0.0) - 5.0).abs() < 1e-10);
        assert!((eval_expr("3 - 1", 0.0, 0.0, 0.0, 0.0) - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_eval_expr_variables() {
        assert!((eval_expr("x", 3.0, 0.0, 0.0, 0.0) - 3.0).abs() < 1e-10);
        assert!((eval_expr("y + z", 0.0, 2.0, 4.0, 0.0) - 6.0).abs() < 1e-10);
        assert!((eval_expr("t", 0.0, 0.0, 0.0, 5.0) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_eval_expr_functions() {
        assert!((eval_expr("sin(0)", 0.0, 0.0, 0.0, 0.0)).abs() < 1e-10);
        assert!((eval_expr("cos(0)", 0.0, 0.0, 0.0, 0.0) - 1.0).abs() < 1e-10);
        assert!((eval_expr("exp(0)", 0.0, 0.0, 0.0, 0.0) - 1.0).abs() < 1e-10);
        assert!((eval_expr("abs(-3)", 0.0, 0.0, 0.0, 0.0) - 3.0).abs() < 1e-10);
        assert!((eval_expr("sqrt(4)", 0.0, 0.0, 0.0, 0.0) - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_eval_expr_constants() {
        let pi = eval_expr("pi", 0.0, 0.0, 0.0, 0.0);
        assert!((pi - std::f64::consts::PI).abs() < 1e-10);
    }

    #[test]
    fn test_eval_expr_div_zero_safe() {
        // Division by zero should return 0 rather than NaN/inf
        let result = eval_expr("1 / 0", 0.0, 0.0, 0.0, 0.0);
        assert!(result.is_finite(), "1/0 should produce finite output, got {}", result);
    }

    #[test]
    fn test_custom_ode_lorenz_like() {
        // Approximate Lorenz: dx=10*(y-x), dy=x*(28-z)-y, dz=x*y-2.667*z
        let mut sys = CustomOde::new(
            "10*(y-x)".into(),
            "x*(28-z)-y".into(),
            "x*y-2.667*z".into(),
        );
        assert_eq!(sys.name(), "custom");
        assert_eq!(sys.dimension(), 3);
        let before: Vec<f64> = sys.state().to_vec();
        sys.step(0.001);
        let after = sys.state();
        assert!(before.iter().zip(after.iter()).any(|(a, b)| (a - b).abs() > 1e-15));
    }

    #[test]
    fn test_custom_ode_state_stays_finite() {
        let mut sys = CustomOde::new(
            "y".into(),
            "-x".into(),
            "0".into(),
        );
        for _ in 0..1000 {
            sys.step(0.01);
        }
        for v in sys.state().iter() {
            assert!(v.is_finite(), "State became non-finite: {}", v);
        }
    }

    #[test]
    fn test_custom_ode_blowup_resets() {
        // An unstable ODE: dx = 1e8 * x should blow up and auto-reset
        let mut sys = CustomOde::new(
            "1e8*x".into(),
            "0".into(),
            "0".into(),
        );
        for _ in 0..100 {
            sys.step(0.1);
        }
        // After blow-up, state should have been reset to finite values
        for v in sys.state().iter() {
            assert!(v.is_finite(), "State should be reset after blowup: {}", v);
        }
    }

    #[test]
    fn test_validate_exprs_ok() {
        let result = validate_exprs("y", "-x", "0", "");
        assert!(result.is_ok(), "Simple harmonic should validate: {:?}", result);
    }

    #[test]
    fn test_validate_exprs_rejects_unknown_ident() {
        let result = validate_exprs("foo*x", "y", "z", "");
        assert!(result.is_err(), "Unknown identifier should produce error");
    }
}
