use crate::BlockSeq;
use crate::Decl;
// use crate::Decl::*;
use crate::Expr;
use crate::IfElseData;
use crate::ParseError;
use crate::Parser;
// use crate::{BinOpType, UnOpType};
use lexer::Token;

impl<'inp> Parser<'inp> {
    pub(crate) fn parse_if_else(&mut self, min_bp: u8) -> Result<Decl, ParseError> {
        dbg!("if else");
        dbg!(&self.lexer.peek());

        // condition - in parens
        // self.consume_token_type(Token::OpenParen, "Expected open parenthesis")?;

        // If token not consumed (no open paren), advance so first token of expr goes into prev_tok
        if !self.consume_opt_token_type(Token::OpenParen) {
            self.advance();
        }

        dbg!("OK", &self.prev_tok);

        let cond = self.parse_expr(min_bp)?.to_expr()?;
        dbg!(&cond); // got cond

        dbg!("peek after:", &self.lexer.peek()); // OpenBrace

        // go past OpenBrace, put in prev_tok
        self.consume_token_type(
            Token::OpenBrace,
            &format!("Expected {} for if block", Token::OpenBrace),
        )?;

        let if_blk = self.parse_blk(min_bp)?.to_block()?;

        dbg!("after parse_blk", &if_blk);

        dbg!("peek after parse_blk", &self.lexer.peek());

        // check else
        let mut else_blk: Option<BlockSeq> = None;

        if self.expect_token_type(Token::Else, "").is_ok() {
            self.consume_token_type(Token::Else, "Expected 'else' for if")?;
            self.consume_token_type(
                Token::OpenBrace,
                &format!("Expected {} for else block", Token::OpenBrace),
            )?;

            let blk = self.parse_blk(min_bp)?.to_block()?;
            dbg!("Got past PARSE ELSE BLK", &blk);

            else_blk.replace(blk);
        }

        let stmt = IfElseData {
            cond,
            if_blk,
            else_blk,
        };

        let expr = Expr::IfElseExpr(Box::new(stmt));

        Ok(Decl::ExprStmt(expr))
    }
}

#[cfg(test)]
mod tests {
    use crate::tests::*;

    #[test]
    fn parse_if_basic() {
        let t = r"
        if (true) {
            30;
            40;
        }
        ";
        test_parse(t, "if true { 30;40; }");

        let t = r"
        if (true) {
            30;
        } else {
            40;
            50
        }
        ";
        test_parse(t, "if true { 30; } else { 40;50 }");

        let t = r"
        if true {
            30;
            40;
        }
        ";
        test_parse(t, "if true { 30;40; }");

        let t = r"
        if (!true) {
            30;
            40;
        }
        ";
        test_parse(t, "if (!true) { 30;40; }");
    }
}
