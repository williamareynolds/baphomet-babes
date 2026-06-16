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
      // In CI the servers run as separate workflow steps (so they can't hang
      // Playwright's teardown); reuse them. Locally Playwright manages them.
      reuseExistingServer: !!process.env.CI,
      timeout: 120_000,
      env: { PATH: `${JAVA_PATH}:${process.env.PATH}` },
    },
    {
      // Backend connects to Firestore at startup — wait for the emulator first.
      command: `bash -c '${WAIT_FOR_EMULATOR}; exec cargo run -p backend'`,
      url: "http://localhost:8080/health",
      cwd: "..",
      // Reuse in CI (pre-started step). Locally never reuse: a dev backend on
      // :8080 would point at REAL Firestore.
      reuseExistingServer: !!process.env.CI,
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
      // Playwright manages trunk itself (it's a single process that tears down
      // cleanly — unlike the emulator's Java child and the cargo-run backend,
      // which are pre-started as workflow steps and reused).
      reuseExistingServer: false,
      timeout: 300_000,
    },
  ],
});
