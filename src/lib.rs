pub mod config;

#[cfg(test)]
mod tests;
mod scheme;

pub use private::LiteClient;
pub use private::Result;
pub use private::DeserializeError;

mod private {
    use std::error::Error;
    use ton_api::ton::TLObject;
    use ton_api::ton::lite_server as lite_result;
    use pretty_hex::PrettyHex;
    use std::fmt::{Display, Formatter};
    use std::net::TcpStream;
    use x25519_dalek::StaticSecret;
    use adnl::{AdnlClient, AdnlBuilder};
    use rand::prelude::SliceRandom;
    use crate::config::ConfigGlobal;
    use crate::scheme;
    use tl_proto::{TlWrite, Bare, TlResult, TlRead};


    #[derive(Debug)]
    pub struct DeserializeError {
        object: TLObject,
    }

    impl Display for DeserializeError {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            write!(f, "Deserialize error, can't downcast {:?}", self.object)
        }
    }

    impl Error for DeserializeError {}

    #[derive(Debug)]
    pub struct LiteError(lite_result::Error);

    impl Into<lite_result::Error> for LiteError {
        fn into(self) -> lite_result::Error {
            self.0
        }
    }

    impl From<lite_result::Error> for LiteError {
        fn from(e: lite_result::Error) -> Self {
            Self(e)
        }
    }

    impl Display for LiteError {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            write!(f, "Server error [code={}]: {}", self.0.code(), self.0.message())
        }
    }

    impl Error for LiteError {}

    pub type Result<T> = std::result::Result<T, Box<dyn Error>>;

    pub struct LiteClient {
        client: AdnlClient<TcpStream>,
    }

    impl LiteClient {
        pub fn connect(config_json: &str) -> Result<Self> {
            let config: ConfigGlobal = serde_json::from_str(config_json)?;
            let ls = config.liteservers.choose(&mut rand::thread_rng()).unwrap();
            let local_secret = StaticSecret::new(rand::rngs::OsRng);
            let transport = TcpStream::connect(ls.socket_addr())?;
            let client = AdnlBuilder::with_random_aes_params(&mut rand::rngs::OsRng)
                .perform_ecdh(local_secret, ls.id.clone())
                .perform_handshake(transport).map_err(|e| format!("{:?}", e))?;
            Ok(Self { client })
        }
        pub fn lite_query<'tl, T, U>(&mut self, request: T, response: &'tl mut Vec<u8>) -> TlResult<U> 
        where 
            T: TlWrite,
            U: TlRead<'tl> 
        {
            let mut message = tl_proto::serialize(scheme::Message::Query { 
                query_id: (scheme::Int256(rand::random())), 
                query: (tl_proto::serialize(scheme::Query{data: (tl_proto::serialize(request))})) 
            });
            
            log::debug!("Sending query:\n{:?}", &message.hex_dump());
            self.client.send(&mut message, &mut rand::random())
                .map_err(|e| format!("{:?}", e)).unwrap();
            log::debug!("Query sent");
            self.client.receive::<_, 8192>(response)
                .map_err(|e| format!("{:?}", e)).unwrap();
            log::debug!("Received:\n{:?}", &response.hex_dump());
            let data = tl_proto::deserialize::<scheme::Message>(response).unwrap();
            // Ok(data)
            if let scheme::Message::Answer { query_id: _, answer} = data {
                *response = answer;
            }
            else {panic!();}
            tl_proto::deserialize::<U>(response)
        }

        pub fn get_masterchain_info(&mut self) -> TlResult<scheme::MasterchainInfo> {
            let  mut response = Vec::<u8>::new();
            self.lite_query(scheme::GetMasterchainInfo, &mut response) as TlResult<scheme::MasterchainInfo> 
        }

        pub fn get_masterchain_info_ext(&mut self) -> TlResult<scheme::MasterchainInfoExt> {
            let  mut response = Vec::<u8>::new();
            self.lite_query(scheme::GetMasterchainInfoExt, &mut response) as TlResult<scheme::MasterchainInfoExt> 
        }
        
        pub fn get_time(&mut self) -> TlResult<scheme::CurrentTime> {
            let  mut response = Vec::<u8>::new();
            self.lite_query(scheme::GetTime, &mut response) as TlResult<scheme::CurrentTime> 
        }

        pub fn get_version(&mut self) -> TlResult<scheme::Version> {
            let  mut response = Vec::<u8>::new();
            self.lite_query(scheme::GetVersion, &mut response) as TlResult<scheme::Version> 
        }

        pub fn get_block(&mut self, id: scheme::BlockIdExt) -> TlResult<scheme::BlockData> {
            let  mut response = Vec::<u8>::new();
            self.lite_query(scheme::GetBlock{id}, &mut response) as TlResult<scheme::BlockData> 
        }
    
        pub fn get_state(&mut self, id: scheme::BlockIdExt) -> TlResult<scheme::BlockState> {
            let  mut response = Vec::<u8>::new();
            self.lite_query(scheme::GetState{id}, &mut response) as TlResult<scheme::BlockState> 
        }

        pub fn get_block_header(&mut self, id: scheme::BlockIdExt, mode: ()) -> TlResult<scheme::BlockHeader> {
            let  mut response = Vec::<u8>::new();
            self.lite_query(scheme::GetBlockHeader{id, mode}, &mut response) as TlResult<scheme::BlockHeader> 
        }

        pub fn send_message(&mut self, body: Vec<u8>) -> TlResult<scheme::SendMsgStatus> {
            let  mut response = Vec::<u8>::new();
            self.lite_query(scheme::SendMessage{body}, &mut response) as TlResult<scheme::SendMsgStatus> 
        }

        pub fn get_account_state(&mut self, id: scheme::BlockIdExt, account: scheme::AccountId) -> TlResult<scheme::AccountState> {
            let  mut response = Vec::<u8>::new();
            self.lite_query(scheme::GetAccountState{id, account}, &mut response) as TlResult<scheme::AccountState> 
        }

        pub fn run_smc_method(&mut self, id: scheme::BlockIdExt, account: scheme::AccountId, method_id: i64, params: Vec<u8>) -> TlResult<scheme::RunMethodResult> {
            let  mut response = Vec::<u8>::new();
            self.lite_query(scheme::RunSmcMethod{mode: (), id, account, method_id, params}, &mut response) as TlResult<scheme::RunMethodResult> 
        }

        pub fn get_shard_info(&mut self, id: scheme::BlockIdExt, workchain: i32, shard: i64, exact: bool) -> TlResult<scheme::ShardInfo> {
            let  mut response = Vec::<u8>::new();
            self.lite_query(scheme::GetShardInfo{id, workchain, shard, exact}, &mut response) as TlResult<scheme::ShardInfo> 
        }

        pub fn get_all_shards_info(&mut self, id: scheme::BlockIdExt) -> TlResult<scheme::AllShardsInfo> {
            let  mut response = Vec::<u8>::new();
            self.lite_query(scheme::GetAllShardsInfo{id}, &mut response) as TlResult<scheme::AllShardsInfo> 
        }

        pub fn get_one_transaction(&mut self, id: scheme::BlockIdExt, account: scheme::AccountId, lt: i64) -> TlResult<scheme::TransactionInfo> {
            let  mut response = Vec::<u8>::new();
            self.lite_query(scheme::GetOneTransaction{id, account, lt}, &mut response) as TlResult<scheme::TransactionInfo> 
        }

        pub fn get_transactions(&mut self, count: i32, account: scheme::AccountId, lt:i64, hash: scheme::Int256) -> TlResult<scheme::TransactionList> {
            let  mut response = Vec::<u8>::new();
            self.lite_query(scheme::GetTransactions{count, account, lt, hash}, &mut response) as TlResult<scheme::TransactionList> 
        }

        pub fn lookup_block(&mut self, id: scheme::BlockId, lt: Option<i64>, utime: Option<i32>) -> TlResult<scheme::BlockHeader> {
            let  mut response = Vec::<u8>::new();
            self.lite_query(scheme::LookupBlock{mode: (), id, lt, utime}, &mut response) as TlResult<scheme::BlockHeader> 
        }

        pub fn list_block_transactions(&mut self, id: scheme::BlockIdExt, count: i32, after: Option<scheme::TransactionId3>, reverse_order: Option<scheme::True>, want_proof: Option<scheme::True>) -> TlResult<scheme::BlockTransactions> {
            let  mut response = Vec::<u8>::new();
            self.lite_query(scheme::ListBlockTransactions{id, mode: (), count, after, reverse_order, want_proof}, &mut response) as TlResult<scheme::BlockTransactions> 
        }

        pub fn get_block_proof(&mut self, known_block: scheme::BlockIdExt, target_block: Option<scheme::BlockIdExt>) -> TlResult<scheme::PartialBlockProof> {
            let  mut response = Vec::<u8>::new();
            self.lite_query(scheme::GetBlockProof{mode: (), known_block, target_block}, &mut response) as TlResult<scheme::PartialBlockProof> 
        }

        pub fn get_config_all(&mut self, id: scheme::BlockIdExt) -> TlResult<scheme::ConfigInfo> {
            let  mut response = Vec::<u8>::new();
            self.lite_query(scheme::GetConfigAll{mode: (), id}, &mut response) as TlResult<scheme::ConfigInfo> 
        }

        pub fn get_config_params(&mut self, id: scheme::BlockIdExt, param_list: Vec<i32>) -> TlResult<scheme::ConfigInfo> {
            let  mut response = Vec::<u8>::new();
            self.lite_query(scheme::GetConfigParams{mode: (), id, param_list}, &mut response) as TlResult<scheme::ConfigInfo> 
        }

        // pub fn get_validator_stats(&mut self, mode: i32, id: BlockIdExt, limit: i32, start_after: Option<[u8; 32]>, modified_after: Option<i32>) -> Result<lite_result::ValidatorStats> {
        //     let start_after = if start_after.is_some() {Some(UInt256::with_array(start_after.unwrap()))} else {None};
        //     self.lite_query(GetValidatorStats{mode, id, limit, start_after, modified_after})
        // }
        pub fn get_validator_stats(&mut self, id: scheme::BlockIdExt, limit: i32, start_after: Option<scheme::Int256>, modified_after: Option<i32>) -> TlResult<scheme::ValidatorStats> {
            let  mut response = Vec::<u8>::new();
            self.lite_query(scheme::GetValidatorStats{mode: (), id, limit, start_after, modified_after}, &mut response) as TlResult<scheme::ValidatorStats> 
        }
    }
}

// 