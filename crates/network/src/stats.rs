use std::{ops::Sub, time::Duration};

use ::util::inspect::Inspect;

#[derive(Default, Inspect, Clone, Copy)]
pub struct ConnectionStatsDiff {
    pub udp_tx: UdpStats,
    pub udp_rx: UdpStats,
    pub frame_tx: FrameStats,
    pub frame_rx: FrameStats,
}

#[derive(Default, Inspect, Clone, Copy)]
pub struct ConnectionStats {
    pub udp_tx: UdpStats,
    pub udp_rx: UdpStats,
    pub frame_tx: FrameStats,
    pub frame_rx: FrameStats,
    pub path: PathStats,
}

impl From<quinn_proto::ConnectionStats> for ConnectionStats {
    fn from(stats: quinn_proto::ConnectionStats) -> Self {
        Self {
            udp_tx: UdpStats {
                datagrams: stats.udp_tx.datagrams,
                bytes: stats.udp_tx.bytes,
                transmits: stats.udp_tx.transmits,
            },
            udp_rx: UdpStats {
                datagrams: stats.udp_rx.datagrams,
                bytes: stats.udp_rx.bytes,
                transmits: stats.udp_rx.transmits,
            },
            frame_tx: FrameStats {
                acks: stats.frame_tx.acks,
                crypto: stats.frame_tx.crypto,
                connection_close: stats.frame_tx.connection_close,
                data_blocked: stats.frame_tx.data_blocked,
                datagram: stats.frame_tx.datagram,
                handshake_done: stats.frame_tx.handshake_done,
                max_data: stats.frame_tx.max_data,
                max_stream_data: stats.frame_tx.max_stream_data,
                max_streams_bidi: stats.frame_tx.max_streams_bidi,
                max_streams_uni: stats.frame_tx.max_streams_uni,
                new_connection_id: stats.frame_tx.new_connection_id,
                new_token: stats.frame_tx.new_token,
                path_challenge: stats.frame_tx.path_challenge,
                path_response: stats.frame_tx.path_response,
                ping: stats.frame_tx.ping,
                reset_stream: stats.frame_tx.reset_stream,
                retire_connection_id: stats.frame_tx.retire_connection_id,
                stream_data_blocked: stats.frame_tx.stream_data_blocked,
                streams_blocked_bidi: stats.frame_tx.streams_blocked_bidi,
                streams_blocked_uni: stats.frame_tx.streams_blocked_uni,
                stop_sending: stats.frame_tx.stop_sending,
                stream: stats.frame_tx.stream,
            },
            frame_rx: FrameStats {
                acks: stats.frame_rx.acks,
                crypto: stats.frame_rx.crypto,
                connection_close: stats.frame_rx.connection_close,
                data_blocked: stats.frame_rx.data_blocked,
                datagram: stats.frame_rx.datagram,
                handshake_done: stats.frame_rx.handshake_done,
                max_data: stats.frame_rx.max_data,
                max_stream_data: stats.frame_rx.max_stream_data,
                max_streams_bidi: stats.frame_rx.max_streams_bidi,
                max_streams_uni: stats.frame_rx.max_streams_uni,
                new_connection_id: stats.frame_rx.new_connection_id,
                new_token: stats.frame_rx.new_token,
                path_challenge: stats.frame_rx.path_challenge,
                path_response: stats.frame_rx.path_response,
                ping: stats.frame_rx.ping,
                reset_stream: stats.frame_rx.reset_stream,
                retire_connection_id: stats.frame_rx.retire_connection_id,
                stream_data_blocked: stats.frame_rx.stream_data_blocked,
                streams_blocked_bidi: stats.frame_rx.streams_blocked_bidi,
                streams_blocked_uni: stats.frame_rx.streams_blocked_uni,
                stop_sending: stats.frame_rx.stop_sending,
                stream: stats.frame_rx.stream,
            },
            path: PathStats {
                rtt: stats.path.rtt,
                cwnd: stats.path.cwnd,
                congestion_events: stats.path.congestion_events,
            },
        }
    }
}

#[derive(Default, Inspect, Clone, Copy)]
pub struct UdpStats {
    pub datagrams: u64,
    pub bytes: u64,
    pub transmits: u64,
}

impl Sub for UdpStats {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            datagrams: self.datagrams - rhs.datagrams,
            bytes: self.bytes - rhs.bytes,
            transmits: self.transmits - rhs.transmits,
        }
    }
}

impl Sub for FrameStats {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            acks: self.acks - rhs.acks,
            crypto: self.crypto - rhs.crypto,
            connection_close: self.connection_close - rhs.connection_close,
            data_blocked: self.data_blocked - rhs.data_blocked,
            datagram: self.datagram - rhs.datagram,
            handshake_done: self.handshake_done - rhs.handshake_done,
            max_data: self.max_data - rhs.max_data,
            max_stream_data: self.max_stream_data - rhs.max_stream_data,
            max_streams_bidi: self.max_streams_bidi - rhs.max_streams_bidi,
            max_streams_uni: self.max_streams_uni - rhs.max_streams_uni,
            new_connection_id: self.new_connection_id - rhs.new_connection_id,
            new_token: self.new_token - rhs.new_token,
            path_challenge: self.path_challenge - rhs.path_challenge,
            path_response: self.path_response - rhs.path_response,
            ping: self.ping - rhs.ping,
            reset_stream: self.reset_stream - rhs.reset_stream,
            retire_connection_id: self.retire_connection_id - rhs.retire_connection_id,
            stream_data_blocked: self.stream_data_blocked - rhs.stream_data_blocked,
            streams_blocked_bidi: self.streams_blocked_bidi - rhs.streams_blocked_bidi,
            streams_blocked_uni: self.streams_blocked_uni - rhs.streams_blocked_uni,
            stop_sending: self.stop_sending - rhs.stop_sending,
            stream: self.stream - rhs.stream,
        }
    }
}

impl Sub for ConnectionStats {
    type Output = ConnectionStatsDiff;

    fn sub(self, rhs: Self) -> Self::Output {
        ConnectionStatsDiff {
            udp_rx: self.udp_rx - rhs.udp_rx,
            udp_tx: self.udp_tx - rhs.udp_tx,
            frame_tx: self.frame_tx - rhs.frame_tx,
            frame_rx: self.frame_rx - rhs.frame_rx,
        }
    }
}

#[derive(Default, Inspect, Clone, Copy)]
pub struct FrameStats {
    pub acks: u64,
    pub crypto: u64,
    pub connection_close: u64,
    pub data_blocked: u64,
    pub datagram: u64,
    pub handshake_done: u8,
    pub max_data: u64,
    pub max_stream_data: u64,
    pub max_streams_bidi: u64,
    pub max_streams_uni: u64,
    pub new_connection_id: u64,
    pub new_token: u64,
    pub path_challenge: u64,
    pub path_response: u64,
    pub ping: u64,
    pub reset_stream: u64,
    pub retire_connection_id: u64,
    pub stream_data_blocked: u64,
    pub streams_blocked_bidi: u64,
    pub streams_blocked_uni: u64,
    pub stop_sending: u64,
    pub stream: u64,
}

#[derive(Default, Clone, Copy)]
pub struct PathStats {
    pub rtt: Duration,
    pub cwnd: u64,
    pub congestion_events: u64,
}

impl Inspect for PathStats {
    fn inspect(&self, name: &str, inspector: &mut impl util::inspect::Inspector) {
        inspector.inspect(name, |i| {
            i.inspect_str("rtt", &format!("{:?}", self.rtt));
            self.cwnd.inspect("cwnd", i);
            self.congestion_events.inspect("congestion_events", i);
        })
    }

    fn inspect_mut(&mut self, name: &str, inspector: &mut impl util::inspect::Inspector) {
        self.inspect(name, inspector)
    }
}
