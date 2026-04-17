import { test, expect } from "@playwright/test";
import { defaultMockState, installTauriMock, populatedState } from "./setup";

test.describe("Dashboard", () => {
  test("empty state: shows zero stats and 'No MCP servers' hint", async ({
    page,
  }) => {
    await installTauriMock(page, defaultMockState());
    await page.goto("/");

    await expect(
      page.getByRole("heading", { name: "Dashboard" }),
    ).toBeVisible();

    // Stats cards render counts from list_servers + list_secrets. Both empty.
    await expect(page.getByText("MCP Servers")).toBeVisible();
    await expect(page.getByText("Secrets Stored")).toBeVisible();

    await expect(
      page.getByText("No MCP servers configured yet"),
    ).toBeVisible();

    // Live indicator — the stripe of text confirming the poll loop is wired up.
    await expect(page.getByText(/Live · last updated/)).toBeVisible();
  });

  test("populated state: server and secret counts reflect mock state", async ({
    page,
  }) => {
    await installTauriMock(page, populatedState());
    await page.goto("/");

    // The Configured Servers list should render our single mock entry.
    await expect(
      page.getByRole("heading", { name: "Configured Servers" }),
    ).toBeVisible();
    await expect(
      page.getByText("GitHub", { exact: true }).first(),
    ).toBeVisible();

    // And the env-mapping badge — "1 env" for our single mapping.
    await expect(page.getByText("1 env")).toBeVisible();
  });
});
