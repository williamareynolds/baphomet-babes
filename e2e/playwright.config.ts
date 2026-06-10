import { defineConfig } from "@playwright/test";

// Java 21+ for the Firestore emulator (harmless prefix if the dir is absent).
const JAVA_PATH = "/usr/local/opt/openjdk/bin";

// Wait for the emulator port without depending on nc being installed.
const WAIT_FOR_EMULATOR =
  "until (echo > /dev/tcp/127.0.0.1/8790) 2>/dev/null; do sleep 1; done";

export default defineConfig({
  testDir: "./tests",
  // The suite builds shared backend state (registered users); keep it ordered.
  workers: 1,
  fullyParallel: false,
  timeout: 60_000,
  expect: { timeout: 10_000 },
  reporter: [["list"]],
  use: {
    baseURL: "http://localhost:3001",
    trace: "retain-on-failure",
  },
  webServer: [
    {
      command:
        "gcloud emulators firestore start --host-port=127.0.0.1:8790 --quiet",
      port: 8790,
      reuseExistingServer: false,
      timeout: 120_000,
      env: { PATH: `${JAVA_PATH}:${process.env.PATH}` },
    },
    {
      // Backend connects to Firestore at startup — wait for the emulator first.
      command: `bash -c '${WAIT_FOR_EMULATOR}; exec cargo run -p backend'`,
      url: "http://localhost:8080/health",
      cwd: "..",
      // Never reuse: a dev backend on :8080 points at REAL Firestore.
      reuseExistingServer: false,
      timeout: 300_000,
      env: {
        FIRESTORE_EMULATOR_HOST: "127.0.0.1:8790",
        GCP_PROJECT_ID: "bb-e2e",
        JWT_SECRET: "e2e-test-secret",
        SUPERADMIN_INVITE_CODE: "e2e-boot-code",
        RUST_LOG: "info",
      },
    },
    {
      command: "trunk serve",
      url: "http://localhost:3001",
      cwd: "../hub",
      reuseExistingServer: false,
      timeout: 300_000,
    },
  ],
});
