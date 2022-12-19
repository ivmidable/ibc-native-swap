import { CosmWasmSigner, Link, testutils, Logger } from "@confio/relayer";
import { fromBase64, fromUtf8 } from "@cosmjs/encoding";
import { SigningCosmWasmClient, Secp256k1HdWallet, GasPrice, Coin, CosmWasmClient } from "cosmwasm";
import { Order } from "cosmjs-types/ibc/core/channel/v1/channel";

import * as fs from 'fs';
import axios from 'axios';
import { ClientRequest } from "http";
import assert, { doesNotMatch } from "assert";

import {
    IbcVersion,
    setupContracts,
    setupOsmosisClient,
    setupOsmosisQueryClient,
    setupWasmClient,
    setupWasmQueryClient,
} from "./utils";

const { osmosis: oldOsmo, setup, wasmd } = testutils;
const osmosis = { ...oldOsmo, minFee: "0.025uosmo" };

let wasmIds: Record<string, number> = {};
let osmosisIds: Record<string, number> = {};

interface SetupInfo {
    wasmClient: CosmWasmSigner;
    osmoClient: CosmWasmSigner;
    wasmSwap: string;
    osmoSwap: string;
    link: Link;
    ics20: {
        wasm: string;
        osmo: string;
    };
    channelIds: {
        wasm: string;
        osmo: string;
    };
};

const logger: Logger = {
    debug(message: string, meta?: Record<string, unknown>): Logger {
      const logMsg = meta ? message + ": " + JSON.stringify(meta) : message;
      console.debug("[relayer|debug]: " + logMsg);
      return this;
    },

    info(message: string, meta?: Record<string, unknown>): Logger {
      const logMsg = meta ? message + ": " + JSON.stringify(meta) : message;
      console.info("[relayer|info]: " + logMsg);
      return this;
    },

    error(message: string, meta?: Record<string, unknown>): Logger {
      const logMsg = meta ? message + ": " + JSON.stringify(meta) : message;
      console.error("[relayer|error]: " + logMsg);
      return this;
    },

    warn(message: string, meta?: Record<string, unknown>): Logger {
      const logMsg = meta ? message + ": " + JSON.stringify(meta) : message;
      console.warn("[relayer|warn]: " + logMsg);
      return this;
    },

    verbose(message: string, meta?: Record<string, unknown>): Logger {
      const logMsg = meta ? message + ": " + JSON.stringify(meta) : message;
      console.debug("[relayer|verbose]: " + logMsg);
      return this;
    },
  };

async function demoSetup(): Promise<SetupInfo> {
    // instantiate ica querier on wasmd
    const wasmClient = await setupWasmClient();
    const { contractAddress: wasmSwap } = await wasmClient.sign.instantiate(
        wasmClient.senderAddress,
        wasmIds.swap,
        { packet_lifetime: 1000 },
        "IBC Swap contract",
        "auto"
    );
    const { ibcPortId: wasmSwapPort } = await wasmClient.sign.getContract(
        wasmSwap
    );
    assert(wasmSwapPort);

    // instantiate ica querier on osmosis
    const osmoClient = await setupOsmosisClient();
    const { contractAddress: osmoSwap } = await osmoClient.sign.instantiate(
        osmoClient.senderAddress,
        osmosisIds.swap,
        { packet_lifetime: 1000 },
        "IBC Swap contract",
        "auto"
    );
    const { ibcPortId: osmoSwapPort } = await osmoClient.sign.getContract(
        osmoSwap
    );
    assert(osmoSwapPort);

    // create a connection and channel for simple-ica
    const [src, dest] = await setup(wasmd, osmosis);
    const link = await Link.createWithNewConnections(src, dest);
    const channelInfo = await link.createChannel(
        "A",
        wasmSwapPort,
        osmoSwapPort,
        Order.ORDER_UNORDERED,
        IbcVersion
    );
    const channelIds = {
        wasm: channelInfo.src.channelId,
        osmo: channelInfo.src.channelId,
    };

    console.log(channelInfo);

    // also create a ics20 channel on this connection
    const ics20Info = await link.createChannel(
        "A",
        wasmd.ics20Port,
        osmosis.ics20Port,
        Order.ORDER_UNORDERED,
        "ics20-1"
    );
    const ics20 = {
        wasm: ics20Info.src.channelId,
        osmo: ics20Info.dest.channelId,
    };
    console.log(ics20Info);

    return {
        wasmClient,
        osmoClient,
        wasmSwap,
        osmoSwap,
        link,
        ics20,
        channelIds,
    };
}

before(async () => {
    console.debug("Upload contracts to wasmd...");
    const wasmContracts = {
        swap: "../artifacts/ibc_native_swap.wasm"
    };
    const wasmSign = await setupWasmClient();
    wasmIds = await setupContracts(wasmSign, wasmContracts);

    console.debug("Upload contracts to osmosis...");
    const osmosisContracts = {
        swap: "../artifacts/ibc_native_swap.wasm",
    };
    const osmosisSign = await setupOsmosisClient();
    osmosisIds = await setupContracts(osmosisSign, osmosisContracts);
});


describe("ibc-native-swapTest", () => {
    it("works", async () => {
        const {
            osmoClient,
            wasmClient,
            wasmSwap,
            osmoSwap,
            link,
            channelIds,
            ics20
        } = await demoSetup();

        const ibcCreate = await wasmClient.sign.execute(
            wasmClient.senderAddress,
            wasmSwap,
            {
                create_swap: {
                    ask: {
                        amount: "1000",
                        denom: { native: "uosmo" }
                    },
                    deposit_transfer_channel_id: ics20.wasm,
                    ask_transfer_channel_id: ics20.osmo
                },
            },
            "auto",
            undefined,
            [{ denom: "ucosm", amount: "1000" }]
        );
        console.log(ibcCreate);

        const info = await link.relayAll();
        console.log(info);
        console.log(fromUtf8(info.acksFromB[0].acknowledgement));

        let wasmQuery = await wasmClient.sign.queryContractSmart(wasmSwap, { get_swap: { side: "A", id: 0 } });
        console.log(wasmQuery)
        let osmoQuery = await osmoClient.sign.queryContractSmart(osmoSwap, { get_swap: { side: "B", id: 0 } });
        console.log(osmoQuery)

        let osmoBlance = await osmoClient.sign.getBalance(osmoClient.senderAddress, "uosmo");
        console.log(osmoBlance);

        const ibcAccept = await osmoClient.sign.execute(
            osmoClient.senderAddress,
            osmoSwap,
            {
                accept_swap: {
                    id: 0
                },
            },
            "auto",
            undefined,
            [{ denom: "uosmo", amount: "1000" }]
        );


        console.log(ibcAccept);

        const accept_info = await link.relayAll();
        console.log(accept_info);
        console.log(fromUtf8(accept_info.acksFromA[0].acknowledgement));

        let wasmQueryClient = await setupWasmQueryClient();
        let osmoQueryClient = await setupOsmosisQueryClient();

        let wasm_allBalances = await wasmQueryClient.getAllBalances(wasmClient.senderAddress);
        console.log(wasm_allBalances);

        let osmo_allBalances = await osmoQueryClient.getAllBalances(osmoClient.senderAddress);
        console.log(osmo_allBalances);

        try {
            let wasmQuery2 = await wasmClient.sign.queryContractSmart(wasmSwap, { get_swap: { side: "A", id: 0 } });
            console.log(wasmQuery2);
        } catch (_) {
            console.log("Swap_a id:0 not found");
        }

        try {
            let osmoQuery2 = await osmoClient.sign.queryContractSmart(osmoSwap, { get_swap: { side: "B", id: 0 } });
            console.log(osmoQuery2);
        } catch (_) {
            console.log("Swap_b id:0 not found");
        }

    });
});