use chia_streamable_macro::{streamable, Streamable};

use crate::Bytes;

#[cfg(feature = "py-bindings")]
use chia_py_streamable_macro::{PyJsonDict, PyStreamable};

#[repr(u8)]
#[cfg_attr(feature = "py-bindings", derive(PyJsonDict, PyStreamable))]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Streamable, Hash, Debug, Copy, Clone, Eq, PartialEq)]
pub enum ProtocolMessageTypes {
    // Shared protocol (all services)
    Handshake = 1,

    // Harvester protocol (harvester <-> farmer)
    HarvesterHandshake = 3,
    // NewSignagePointHarvester = 4 Changed to 66 in new protocol
    NewProofOfSpace = 5,
    RequestSignatures = 6,
    RespondSignatures = 7,

    // Farmer protocol (farmer <-> fullNode)
    NewSignagePoint = 8,
    DeclareProofOfSpace = 9,
    RequestSignedValues = 10,
    SignedValues = 11,
    FarmingInfo = 12,

    // Timelord protocol (timelord <-> fullNode)
    NewPeakTimelord = 13,
    NewUnfinishedBlockTimelord = 14,
    NewInfusionPointVdf = 15,
    NewSignagePointVdf = 16,
    NewEndOfSubSlotVdf = 17,
    RequestCompactProofOfTime = 18,
    RespondCompactProofOfTime = 19,

    // Full node protocol (fullNode <-> fullNode)
    NewPeak = 20,
    NewTransaction = 21,
    RequestTransaction = 22,
    RespondTransaction = 23,
    RequestProofOfWeight = 24,
    RespondProofOfWeight = 25,
    RequestBlock = 26,
    RespondBlock = 27,
    RejectBlock = 28,
    RequestBlocks = 29,
    RespondBlocks = 30,
    RejectBlocks = 31,
    NewUnfinishedBlock = 32,
    RequestUnfinishedBlock = 33,
    RespondUnfinishedBlock = 34,
    NewSignagePointOrEndOfSubSlot = 35,
    RequestSignagePointOrEndOfSubSlot = 36,
    RespondSignagePoint = 37,
    RespondEndOfSubSlot = 38,
    RequestMempoolTransactions = 39,
    RequestCompactVDF = 40,
    RespondCompactVDF = 41,
    NewCompactVDF = 42,
    RequestPeers = 43,
    RespondPeers = 44,
    NoneResponse = 91,

    // Wallet protocol (wallet <-> fullNode)
    RequestPuzzleSolution = 45,
    RespondPuzzleSolution = 46,
    RejectPuzzleSolution = 47,
    SendTransaction = 48,
    TransactionAck = 49,
    NewPeakWallet = 50,
    RequestBlockHeader = 51,
    RespondBlockHeader = 52,
    RejectHeaderRequest = 53,
    RequestRemovals = 54,
    RespondRemovals = 55,
    RejectRemovalsRequest = 56,
    RequestAdditions = 57,
    RespondAdditions = 58,
    RejectAdditionsRequest = 59,
    RequestHeaderBlocks = 60,
    RejectHeaderBlocks = 61,
    RespondHeaderBlocks = 62,

    // Introducer protocol (introducer <-> fullNode)
    RequestPeersIntroducer = 63,
    RespondPeersIntroducer = 64,

    // Simulator protocol
    FarmNewBlock = 65,

    // New harvester protocol
    NewSignagePointHarvester = 66,
    RequestPlots = 67,
    RespondPlots = 68,
    PlotSyncStart = 78,
    PlotSyncLoaded = 79,
    PlotSyncRemoved = 80,
    PlotSyncInvalid = 81,
    PlotSyncKeysMissing = 82,
    PlotSyncDuplicates = 83,
    PlotSyncDone = 84,
    PlotSyncResponse = 85,

    // More wallet protocol
    CoinStateUpdate = 69,
    RegisterForPhUpdates = 70,
    RespondToPhUpdates = 71,
    RegisterForCoinUpdates = 72,
    RespondToCoinUpdates = 73,
    RequestChildren = 74,
    RespondChildren = 75,
    RequestSesInfo = 76,
    RespondSesInfo = 77,
    RequestBlockHeaders = 86,
    RejectBlockHeaders = 87,
    RespondBlockHeaders = 88,
    RequestFeeEstimates = 89,
    RespondFeeEstimates = 90,

    // Unfinished block protocol
    NewUnfinishedBlock2 = 92,
    RequestUnfinishedBlock2 = 93,

    // New wallet sync protocol
    RequestRemovePuzzleSubscriptions = 94,
    RespondRemovePuzzleSubscriptions = 95,
    RequestRemoveCoinSubscriptions = 96,
    RespondRemoveCoinSubscriptions = 97,
    RequestPuzzleState = 98,
    RespondPuzzleState = 99,
    RejectPuzzleState = 100,
    RequestCoinState = 101,
    RespondCoinState = 102,
    RejectCoinState = 103,
}

#[cfg(feature = "py-bindings")]
impl chia_traits::ChiaToPython for ProtocolMessageTypes {
    fn to_python<'a>(&self, py: pyo3::Python<'a>) -> pyo3::PyResult<pyo3::Bound<'a, pyo3::PyAny>> {
        Ok(pyo3::IntoPy::into_py(*self, py).bind(py).clone().into_any())
    }
}

pub trait ChiaProtocolMessage {
    fn msg_type() -> ProtocolMessageTypes;
}

#[repr(u8)]
#[cfg_attr(feature = "py-bindings", derive(PyJsonDict, PyStreamable))]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Streamable, Hash, Debug, Copy, Clone, Eq, PartialEq)]
pub enum NodeType {
    FullNode = 1,
    Harvester = 2,
    Farmer = 3,
    Timelord = 4,
    Introducer = 5,
    Wallet = 6,
    DataLayer = 7,
}

#[cfg(feature = "py-bindings")]
impl chia_traits::ChiaToPython for NodeType {
    fn to_python<'a>(&self, py: pyo3::Python<'a>) -> pyo3::PyResult<pyo3::Bound<'a, pyo3::PyAny>> {
        Ok(pyo3::IntoPy::into_py(*self, py).bind(py).clone().into_any())
    }
}

#[streamable]
pub struct Message {
    msg_type: ProtocolMessageTypes,
    id: Option<u16>,
    data: Bytes,
}

#[streamable(message)]
pub struct Handshake {
    // Network id, usually the genesis challenge of the blockchain
    network_id: String,
    // Protocol version to determine which messages the peer supports
    protocol_version: String,
    // Version of the software, to debug and determine feature support
    software_version: String,
    // Which port the server is listening on
    server_port: u16,
    // NodeType (full node, wallet, farmer, etc.)
    node_type: NodeType,
    // Key value dict to signal support for additional capabilities/features
    capabilities: Vec<(u16, String)>,
}
