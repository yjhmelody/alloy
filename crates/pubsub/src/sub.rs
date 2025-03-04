use alloy_primitives::B256;
use serde::de::DeserializeOwned;
use serde_json::value::RawValue;
use tokio::sync::broadcast;

/// A Subscription is a feed of notifications from the server, identified by a
/// local ID.
///
/// This type is mostly a wrapper around [`broadcast::Receiver`], and exposes
/// the same methods.
#[derive(Debug)]
pub struct RawSubscription {
    /// The channel via which notifications are received.
    pub(crate) rx: broadcast::Receiver<Box<RawValue>>,
    /// The local ID of the subscription.
    pub(crate) local_id: B256,
}

impl RawSubscription {
    /// Get the local ID of the subscription.
    pub const fn local_id(&self) -> B256 {
        self.local_id
    }

    /// Wrapper for [`blocking_recv`]. Block the current thread until a message
    /// is available.
    ///
    /// [`blocking_recv`]: broadcast::Receiver::blocking_recv
    pub fn blocking_recv(&mut self) -> Result<Box<RawValue>, broadcast::error::RecvError> {
        self.rx.blocking_recv()
    }

    /// Returns `true` if the broadcast channel is empty (i.e. there are
    /// currently no notifications to receive).
    pub fn is_empty(&self) -> bool {
        self.rx.is_empty()
    }

    /// Returns the number of messages in the broadcast channel that this
    /// receiver has yet to receive.
    pub fn len(&self) -> usize {
        self.rx.len()
    }

    /// Wrapper for [`recv`]. Await an item from the channel.
    ///
    /// [`recv`]: broadcast::Receiver::recv
    pub async fn recv(&mut self) -> Result<Box<RawValue>, broadcast::error::RecvError> {
        self.rx.recv().await
    }

    /// Wrapper for [`resubscribe`]. Create a new Subscription, starting from
    /// the current tail element.
    ///
    /// [`resubscribe`]: broadcast::Receiver::resubscribe
    pub fn resubscribe(&self) -> Self {
        Self { rx: self.rx.resubscribe(), local_id: self.local_id }
    }

    /// Wrapper for [`same_channel`]. Returns `true` if the two subscriptions
    /// share the same broadcast channel.
    ///
    /// [`same_channel`]: broadcast::Receiver::same_channel
    pub fn same_channel(&self, other: &Self) -> bool {
        self.rx.same_channel(&other.rx)
    }

    /// Wrapper for [`try_recv`]. Attempt to receive a message from the channel
    /// without awaiting.
    ///
    /// [`try_recv`]: broadcast::Receiver::try_recv
    pub fn try_recv(&mut self) -> Result<Box<RawValue>, broadcast::error::TryRecvError> {
        self.rx.try_recv()
    }
}

#[derive(Debug)]
/// An item in a typed [`Subscription`]. This is either the expected type, or
/// some serialized value of another type.
pub enum SubscriptionItem<T> {
    /// The expected item.
    Item(T),
    /// Some other value.
    Other(Box<RawValue>),
}

impl<T: DeserializeOwned> From<Box<RawValue>> for SubscriptionItem<T> {
    fn from(value: Box<RawValue>) -> Self {
        if let Ok(item) = serde_json::from_str(value.get()) {
            SubscriptionItem::Item(item)
        } else {
            trace!(value = value.get(), "Received unexpected value in subscription.");
            SubscriptionItem::Other(value)
        }
    }
}

/// A Subscription is a feed of notifications from the server of a specific
/// type `T`, identified by a local ID.
///
/// For flexibility, we expose three similar APIs:
/// - The [`Subscription::recv`] method and its variants will discard any notifications of
///   unexpected types.
/// - The [`Subscription::recv_any`] and its variants will yield unexpected types as
///   [`SubscriptionItem::Other`].
/// - The [`Subscription::recv_result`] and its variants will attempt to deserialize the
///  notifications and yield the `serde_json::Result` of the deserialization.
#[derive(Debug)]
pub struct Subscription<T> {
    pub(crate) inner: RawSubscription,
    _pd: std::marker::PhantomData<T>,
}

impl<T> From<RawSubscription> for Subscription<T> {
    fn from(inner: RawSubscription) -> Self {
        Self { inner, _pd: std::marker::PhantomData }
    }
}

impl<T> Subscription<T> {
    /// Get the local ID of the subscription.
    pub const fn local_id(&self) -> B256 {
        self.inner.local_id()
    }

    /// Convert the subscription into its inner [`RawSubscription`].
    #[allow(clippy::missing_const_for_fn)] // erroneous lint
    pub fn into_raw(self) -> RawSubscription {
        self.inner
    }

    /// Get a reference to the inner subscription.
    pub const fn inner(&self) -> &RawSubscription {
        &self.inner
    }

    /// Get a mutable reference to the inner subscription.
    pub fn inner_mut(&mut self) -> &mut RawSubscription {
        &mut self.inner
    }

    /// Returns `true` if the broadcast channel is empty (i.e. there are
    /// currently no notifications to receive).
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns the number of messages in the broadcast channel that this
    /// receiver has yet to receive.
    ///
    /// NB: This count may include messages of unexpected types that will be
    /// discarded upon receipt.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Wrapper for [`resubscribe`]. Create a new [`RawSubscription`], starting
    /// from the current tail element.
    ///
    /// [`resubscribe`]: broadcast::Receiver::resubscribe
    pub fn resubscribe_inner(&self) -> RawSubscription {
        self.inner.resubscribe()
    }

    /// Wrapper for [`resubscribe`]. Create a new `Subscription`, starting from
    /// the current tail element.
    ///
    /// [`resubscribe`]: broadcast::Receiver::resubscribe
    pub fn resubscribe(&self) -> Self {
        self.inner.resubscribe().into()
    }

    /// Wrapper for [`same_channel`]. Returns `true` if the two subscriptions
    /// share the same broadcast channel.
    ///
    /// [`same_channel`]: broadcast::Receiver::same_channel
    pub fn same_channel<U>(&self, other: &Subscription<U>) -> bool {
        self.inner.same_channel(&other.inner)
    }
}

impl<T: DeserializeOwned> Subscription<T> {
    /// Wrapper for [`blocking_recv`], may produce unexpected values. Block the
    /// current thread until a message is available.
    ///
    /// [`blocking_recv`]: broadcast::Receiver::blocking_recv
    pub fn blocking_recv_any(
        &mut self,
    ) -> Result<SubscriptionItem<T>, broadcast::error::RecvError> {
        self.inner.blocking_recv().map(Into::into)
    }

    /// Wrapper for [`recv`], may produce unexpected values. Await an item from
    /// the channel.
    ///
    /// [`recv`]: broadcast::Receiver::recv
    pub async fn recv_any(&mut self) -> Result<SubscriptionItem<T>, broadcast::error::RecvError> {
        self.inner.recv().await.map(Into::into)
    }

    /// Wrapper for [`try_recv`]. Attempt to receive a message from the channel
    /// without awaiting.
    ///
    /// [`try_recv`]: broadcast::Receiver::try_recv
    pub fn try_recv_any(&mut self) -> Result<SubscriptionItem<T>, broadcast::error::TryRecvError> {
        self.inner.try_recv().map(Into::into)
    }

    /// Wrapper for [`blocking_recv`]. Block the current thread until a message
    /// of the expected type is available.
    ///
    /// [`blocking_recv`]: broadcast::Receiver::blocking_recv
    pub fn blocking_recv(&mut self) -> Result<T, broadcast::error::RecvError> {
        loop {
            match self.blocking_recv_any()? {
                SubscriptionItem::Item(item) => return Ok(item),
                SubscriptionItem::Other(_) => continue,
            }
        }
    }

    /// Wrapper for [`recv`]. Await an item of the expected type from the
    /// channel.
    ///
    /// [`recv`]: broadcast::Receiver::recv
    pub async fn recv(&mut self) -> Result<T, broadcast::error::RecvError> {
        loop {
            match self.recv_any().await? {
                SubscriptionItem::Item(item) => return Ok(item),
                SubscriptionItem::Other(_) => continue,
            }
        }
    }

    /// Wrapper for [`try_recv`]. Attempt to receive a message of the expected
    /// type from the channel without awaiting.
    ///
    /// [`try_recv`]: broadcast::Receiver::try_recv
    pub fn try_recv(&mut self) -> Result<T, broadcast::error::TryRecvError> {
        loop {
            match self.try_recv_any()? {
                SubscriptionItem::Item(item) => return Ok(item),
                SubscriptionItem::Other(_) => continue,
            }
        }
    }

    /// Wrapper for [`blocking_recv`]. Block the current thread until a message
    /// is available, deserializing the message and returning the result.
    ///
    /// [`blocking_recv`]: broadcast::Receiver::blocking_recv
    pub fn blocking_recv_result(
        &mut self,
    ) -> Result<Result<T, serde_json::Error>, broadcast::error::RecvError> {
        self.inner.blocking_recv().map(|value| serde_json::from_str(value.get()))
    }

    /// Wrapper for [`recv`]. Await an item from the channel, deserializing the
    /// message and returning the result.
    ///
    /// [`recv`]: broadcast::Receiver::recv
    pub async fn recv_result(
        &mut self,
    ) -> Result<Result<T, serde_json::Error>, broadcast::error::RecvError> {
        self.inner.recv().await.map(|value| serde_json::from_str(value.get()))
    }

    /// Wrapper for [`try_recv`]. Attempt to receive a message from the channel
    /// without awaiting, deserializing the message and returning the result.
    ///
    /// [`try_recv`]: broadcast::Receiver::try_recv
    pub fn try_recv_result(
        &mut self,
    ) -> Result<Result<T, serde_json::Error>, broadcast::error::TryRecvError> {
        self.inner.try_recv().map(|value| serde_json::from_str(value.get()))
    }
}
