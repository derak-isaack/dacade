import React from "react";
import { AuthClient } from "@dfinity/auth-client";
import { HttpAgent } from "@dfinity/agent";

const LoginButton = () => {
  // Function to handle login
  const login = async () => {
    const authClient = await AuthClient.create();

    // Get the Internet Identity URL
    const iiUrl =
      process.env.DFX_NETWORK === "ic"
        ? "https://identity.ic0.app"
        : `http://localhost:4943/?canisterId=${process.env.CANISTER_ID_INTERNET_ID}`;

    await new Promise((resolve, reject) => {
      authClient.login({
        identityProvider: iiUrl,
        onSuccess: resolve,
        onError: reject,
      });
    });

    const identity = authClient.getIdentity();
    const agent = new HttpAgent({ identity });

    console.log("Logged in with principal:", identity.getPrincipal().toText());

    // Update login status
    const statusElement = document.getElementById("loginStatus");
    if (statusElement) {
      statusElement.innerText = "Logged in";
    }
  };

  return (
    <div>
      <button id="loginBtn" onClick={login}>
        Login
      </button>
      <p id="loginStatus">Not logged in</p>
    </div>
  );
};

export default LoginButton;
