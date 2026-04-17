import { test, expect } from "@playwright/test";
import { defaultMockState, installTauriMock, populatedState } from "./setup";

test.describe("Config", () => {
  test("copy warns when export includes untrusted servers", async ({ page }) => {
    await installTauriMock(page, populatedState());
    await page.goto("/config");

    await expect(
      page.getByText("Untrusted servers will be exported"),
    ).toBeVisible();

    await page.getByRole("button", { name: /copy/i }).click();
    await expect(
      page.getByRole("heading", { name: /untrusted server warning/i }),
    ).toBeVisible();
    await expect(page.getByText("GitHub", { exact: true }).first()).toBeVisible();
    await expect(page.getByText("GitHub PAT", { exact: true })).toBeVisible();
  });

  test("trusted export writes without untrusted warning", async ({ page }) => {
    await installTauriMock(
      page,
      defaultMockState({
        servers: [
          {
            id: "github",
            name: "GitHub",
            command: "npx",
            args: ["-y", "@modelcontextprotocol/server-github"],
            transport: { type: "Stdio" },
            env_mappings: [
              { env_var_name: "GITHUB_TOKEN", secret_ref: "github-pat" },
            ],
            run_mode: { type: "Local" },
            enabled: true,
            trusted: true,
            created_at: "2025-01-01T00:00:00Z",
            updated_at: "2025-01-01T00:00:00Z",
          },
        ],
        secrets: [
          { id: "github-pat", label: "GitHub PAT", source: { type: "Local" } },
        ],
      }),
    );
    await page.goto("/config");

    await expect(
      page.getByText("Untrusted servers will be exported"),
    ).toBeHidden();

    await page.getByRole("button", { name: /write to file/i }).click();
    await expect(
      page.getByRole("heading", { name: /write to claude desktop/i }),
    ).toBeVisible();
    await expect(
      page.getByRole("heading", { name: /untrusted server warning/i }),
    ).toBeHidden();
  });
});
