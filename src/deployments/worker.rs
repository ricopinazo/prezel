use std::{future::Future, sync::Arc};

use tokio::sync::{
    mpsc::{channel, Sender},
    Notify,
};

// i need to split this in two or smth
//
// I need to be able to first get something I can send messages to
//
//
//
//

// I can have a work queue
// and then subscrube some worker to it!!
// I could even have three different structs
//
// do I really need that?

pub(crate) trait Worker: Sized + Sync + Send + 'static {
    fn start<F: FnOnce(WorkerHandle) -> Self>(constructor: F) -> WorkerHandle {
        let (sender, receiver) = channel::<Arc<Notify>>(1000); // TODO: review this size
        let handle = WorkerHandle { sender };
        let worker = constructor(handle.clone());

        tokio::spawn(async move {
            let mut receiver = receiver;
            loop {
                let mut notifies = vec![];
                receiver.recv_many(&mut notifies, 100).await;
                worker.work().await;
                for notify in notifies {
                    notify.notify_one();
                }
            }
        });

        handle
    }

    fn work(&self) -> impl Future<Output = ()> + Send;
}

#[derive(Debug, Clone)]
pub(crate) struct WorkerHandle {
    sender: Sender<Arc<Notify>>,
}

impl WorkerHandle {
    pub(crate) fn trigger(&self) {
        let _ = self.sender.try_send(Notify::new().into());
    }

    pub(crate) async fn trigger_and_wait(&self) {
        let notify: Arc<_> = Notify::new().into();
        let _ = self.sender.try_send(notify.clone()); // FIXME: stop ignoring errors here?
        notify.notified().await;
    }
}
