use scoop_hir as hir;

#[derive(Clone)]
pub(crate) struct PropertyCallReceiver {
    pub(crate) value: hir::Expr,
    pub(crate) static_type: hir::TypeId,
}

impl PropertyCallReceiver {
    pub(crate) fn source_type(receiver: &Option<Self>) -> hir::SourceCallReceiver<hir::TypeId> {
        match receiver {
            Some(receiver) => hir::SourceCallReceiver::Receiver {
                static_type: receiver.static_type,
            },
            None => hir::SourceCallReceiver::NoReceiver,
        }
    }
}
