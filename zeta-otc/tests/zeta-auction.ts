import * as anchor from "@project-serum/anchor";
import { Program } from "@project-serum/anchor";
import { ZetaAuction } from "../target/types/zeta_auction";

describe("zeta-auction", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.Provider.env());

  it("Uses the workspace to invoke the initialize instruction", async () => {
    // #region code
    // Read the deployed program from the workspace.
    const program = anchor.workspace.ZetaAuction as Program<ZetaAuction>;
    console.log("program = ", program);

    // Execute the RPC.
    await program.rpc.initialize();
    // #endregion code
  });
});
