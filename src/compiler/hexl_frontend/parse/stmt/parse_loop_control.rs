//! stmt.rsのparse_loop関数でエラーをそのまま返すか
//! エラープールに入れるかpanicするかを制御するトレイト
use super::*;
use crate::err;

pub(in crate::compiler::hexl_frontend::parse) trait ParseLoopControl {
    fn handle(
        err_pool: &Rc<RefCell<err::err_pool::ErrPool>>,
        result: Result<(), err::ErrKind>,
    ) -> Result<(), err::ErrKind>;
    fn handle_ty<R: std::fmt::Debug>(
        err_pool: &Rc<RefCell<err::err_pool::ErrPool>>,
        result: Result<R, err::ErrKind>,
    ) -> Result<R, err::ErrKind>;
}

/// でバック用
pub(in crate::compiler::hexl_frontend::parse) struct ParseLoopPanic {}

impl ParseLoopControl for ParseLoopPanic {
    fn handle(
        err_pool: &Rc<RefCell<err::err_pool::ErrPool>>,
        result: Result<(), err::ErrKind>,
    ) -> Result<(), err::ErrKind> {
        if result.is_err() {
            panic!("{:?}", result);
        }
        result
    }

    fn handle_ty<R: std::fmt::Debug>(
        err_pool: &Rc<RefCell<err::err_pool::ErrPool>>,
        result: Result<R, err::ErrKind>,
    ) -> Result<R, err::ErrKind> {
        if result.is_err() {
            panic!("{:?}", result);
        }
        result
    }
}

/// 表準でこっちを使う
pub(in crate::compiler::hexl_frontend::parse) struct ParseLoopPushPool {}

/// エラーをエラープールに挿入する
impl ParseLoopControl for ParseLoopPushPool {
    fn handle(
        err_pool: &Rc<RefCell<err::err_pool::ErrPool>>,
        result: Result<(), err::ErrKind>,
    ) -> Result<(), err::ErrKind> {
        if let Err(ref e) = result {
            (*Rc::clone(err_pool).borrow_mut()).push_err_node(e.clone())
        }
        result
    }

    fn handle_ty<R: std::fmt::Debug>(
        err_pool: &Rc<RefCell<err::err_pool::ErrPool>>,
        result: Result<R, err::ErrKind>,
    ) -> Result<R, err::ErrKind> {
        if let Err(ref e) = result {
            (*Rc::clone(err_pool).borrow_mut()).push_err_node(e.clone())
        }
        result
    }
}
