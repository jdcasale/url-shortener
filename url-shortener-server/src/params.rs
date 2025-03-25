use clap::Parser;

#[derive(Parser)]
pub struct Args {
    #[clap(long)]
    pub(crate) node_id: u64,
    #[clap(long)]
    pub(crate) raft_rpc_addr: String,
    #[clap(long)]
    pub(crate) http_addr: String,
    #[clap(long)]
    pub(crate) data_dir: String,
}