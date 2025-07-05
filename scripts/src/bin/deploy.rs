use av_event_helpers::get_license_fee;
use cw_ave_factory::msg::InstantiateMsg;
use cw_orch::{
    daemon::networks::LOCAL_JUNO,
    prelude::{networks::ChainInfo, *},
};
use scripts::interfaces::CwAveSuite;

fn full_deploy(networks: Vec<ChainInfo>) -> cw_orch::anyhow::Result<()> {
    for network in networks {
        let chain = DaemonBuilder::new(network.clone()).build()?;

        let suite = CwAveSuite::deploy_on(chain.clone(), ())?;

        // instantiate factory
        suite.cw_ave_factory.instantiate(
            &InstantiateMsg {
                owner: None,
                cw_ave_id: suite.cw_ave.code_id()?,
            },
            Some(&chain.sender_addr()),
            &[get_license_fee(&chain.chain_info().chain_id)?],
        )?;

        
    }

    Ok(())
}

fn main() {
    dotenv().ok();
    env_logger::init();

    use dotenv::dotenv;

    let networks = vec![LOCAL_JUNO];
    full_deploy(networks).unwrap();
}
