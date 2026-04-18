import { test, expect } from "@playwright/test";
import { defaultMockState, installTauriMock, populatedState } from "./setup";

test.describe("Settings", () => {
  test("renders recent audit log entries", async ({ page }) => {
    await installTauriMock(page, populatedState());
    await page.goto("/settings");

    await expect(
      page.getByRole("heading", { name: "Audit Log" }),
    ).toBeVisible();
    await expect(page.getByText("github", { exact: true })).toBeVisible();
    await expect(page.getByText("github-pat", { exact: true })).toBeVisible();
    await expect(page.getByText("Success", { exact: true })).toBeVisible();
  });

  test("shows empty audit log state", async ({ page }) => {
    await installTauriMock(page, defaultMockState());
    await page.goto("/settings");

    await expect(
      page.getByText(/No audit log entries yet/i),
    ).toBeVisible();
  });

  test("macOS user can switch from Keychain to the Local Vault with a warning", async ({
    page,
  }) => {
    // Default mock state is Keychain + can_switch_backend: true.
    await installTauriMock(page, defaultMockState());
    await page.goto("/settings");

    // Capture invoke calls so we can assert `set_prefer_local_vault` fires.
    const invokeCalls: Array<{ cmd: string; args: unknown }> = [];
    await page.exposeFunction(
      "__recordInvoke__",
      (cmd: string, args: unknown) => {
        invokeCalls.push({ cmd, args });
      },
    );
    await page.evaluate(() => {
      const internals = (
        window as unknown as {
          __TAURI_INTERNALS__: {
            invoke: (cmd: string, args: Record<string, unknown>) => unknown;
          };
        }
      ).__TAURI_INTERNALS__;
      const original = internals.invoke.bind(internals);
      internals.invoke = (cmd: string, args: Record<string, unknown>) => {
        (
          window as unknown as {
            __recordInvoke__: (c: string, a: unknown) => void;
          }
        ).__recordInvoke__(cmd, args);
        return original(cmd, args);
      };
    });

    // Click "Switch to Local Vault" — the pill only exists when can_switch_backend is true.
    await page
      .getByRole("button", { name: /switch to local vault/i })
      .first()
      .click();

    // Confirmation modal must appear with the migration-warning copy.
    await expect(
      page.getByRole("heading", {
        name: /switch local backend to local vault/i,
      }),
    ).toBeVisible();
    // Modal uses distinctive phrasing "Switching backends does not migrate"
    // while the card says "Switching does not migrate" — match just the
    // modal copy to avoid ambiguity with the card hint.
    await expect(
      page.getByText(/switching backends does/i),
    ).toBeVisible();

    // Confirm.
    await page
      .getByRole("button", { name: /confirm switch to local vault/i })
      .click();

    // The toggle must invoke the backend with enabled=true.
    await expect
      .poll(() =>
        invokeCalls.find((c) => c.cmd === "set_prefer_local_vault"),
      )
      .toMatchObject({
        cmd: "set_prefer_local_vault",
        args: { enabled: true },
      });

    // After confirmation the Security card should now reflect the vault
    // backend, and a fresh "Switch to macOS Keychain" button should appear.
    await expect(
      page.getByRole("button", { name: /switch to macos keychain/i }),
    ).toBeVisible();
  });

  test("switch back to Keychain is disabled while the vault is locked", async ({
    page,
  }) => {
    await installTauriMock(
      page,
      defaultMockState({
        vault: {
          backend: "encrypted-file",
          exists: true,
          unlocked: false,
          prefer_local_vault: true,
          can_switch_backend: true,
        },
      }),
    );
    await page.goto("/settings");

    const switchBtn = page.getByRole("button", {
      name: /switch to macos keychain/i,
    });
    await expect(switchBtn).toBeVisible();
    await expect(switchBtn).toBeDisabled();
    await expect(
      page.getByText(/unlock the vault before switching back/i),
    ).toBeVisible();
  });

  test("switch affordance is hidden when the platform can't switch", async ({
    page,
  }) => {
    // Non-macOS simulation: can_switch_backend=false means no UI shows up.
    await installTauriMock(
      page,
      defaultMockState({
        vault: {
          backend: "encrypted-file",
          exists: false,
          unlocked: false,
          prefer_local_vault: false,
          can_switch_backend: false,
        },
      }),
    );
    await page.goto("/settings");

    await expect(
      page.getByRole("button", { name: /switch to/i }),
    ).toHaveCount(0);
  });
});
