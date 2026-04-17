import { test, expect } from "@playwright/test";
import { defaultMockState, installTauriMock, populatedState } from "./setup";

test.describe("Servers", () => {
  test("add server via manual form", async ({ page }) => {
    await installTauriMock(page, defaultMockState());
    await page.goto("/servers");

    await expect(
      page.getByText("No MCP servers configured yet"),
    ).toBeVisible();

    await page.getByRole("button", { name: /add server/i }).click();

    // The modal doesn't use htmlFor/id pairs — target by placeholder.
    await page.getByPlaceholder("e.g., GitHub MCP").fill("GitHub");
    await page.getByPlaceholder("e.g., npx").fill("npx");
    await page
      .getByPlaceholder("e.g., -y @modelcontextprotocol/server-github")
      .fill("-y @modelcontextprotocol/server-github");

    await page.getByRole("button", { name: /save server/i }).click();

    // After save → modal closes, server appears as a card.
    await expect(page.getByText("GitHub", { exact: true })).toBeVisible();
    await expect(
      page.getByText("No MCP servers configured yet"),
    ).toBeHidden();
  });

  test("edit server updates command", async ({ page }) => {
    await installTauriMock(page, populatedState());
    await page.goto("/servers");

    await expect(page.getByText("GitHub", { exact: true })).toBeVisible();

    await page.getByRole("button", { name: "Edit GitHub" }).click();

    // Pre-filled. Change command from `npx` to `bun`.
    const commandInput = page.getByPlaceholder("e.g., npx");
    await expect(commandInput).toHaveValue("npx");
    await commandInput.fill("bun");

    await page.getByRole("button", { name: /update server/i }).click();

    // Card's mono command line should now start with `bun`.
    await expect(
      page
        .locator("p.font-mono")
        .filter({ hasText: /bun -y @modelcontextprotocol\/server-github/ }),
    ).toBeVisible();
  });

  test("delete server removes it from the list", async ({ page }) => {
    await installTauriMock(page, populatedState());
    await page.goto("/servers");

    await expect(page.getByText("GitHub", { exact: true })).toBeVisible();

    await page.getByRole("button", { name: "Delete GitHub" }).click();

    // After delete → list is empty again.
    await expect(
      page.getByText("No MCP servers configured yet"),
    ).toBeVisible();
  });

  test("search narrows the list", async ({ page }) => {
    await installTauriMock(
      page,
      defaultMockState({
        servers: [
          {
            id: "github",
            name: "GitHub",
            command: "npx",
            args: [],
            transport: { type: "Stdio" },
            env_mappings: [],
            run_mode: { type: "Local" },
            enabled: true,
            trusted: false,
            created_at: "2025-01-01T00:00:00Z",
            updated_at: "2025-01-01T00:00:00Z",
          },
          {
            id: "fs",
            name: "Filesystem",
            command: "node",
            args: [],
            transport: { type: "Stdio" },
            env_mappings: [],
            run_mode: { type: "Local" },
            enabled: true,
            trusted: false,
            created_at: "2025-01-01T00:00:00Z",
            updated_at: "2025-01-01T00:00:00Z",
          },
        ],
      }),
    );
    await page.goto("/servers");

    await expect(page.getByText("GitHub", { exact: true })).toBeVisible();
    await expect(page.getByText("Filesystem", { exact: true })).toBeVisible();

    await page
      .getByPlaceholder("Search by name, command, image…")
      .fill("github");

    await expect(page.getByText("GitHub", { exact: true })).toBeVisible();
    await expect(page.getByText("Filesystem", { exact: true })).toBeHidden();
  });
});
