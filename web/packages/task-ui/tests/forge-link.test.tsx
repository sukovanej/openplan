import { act } from "react"
import { describe, expect, it } from "vitest"

import { ForgeLink } from "../src/forge-link"
import { PullRequestsMark } from "../src/pull-requests-mark"
import { render } from "./render"

const OWN = "https://github.com/acme/widgets/pull/214"

describe("ForgeLink", () => {
  it("shows the icon of the forge and the short form, and keeps the whole address", () => {
    const root = render(<ForgeLink url={OWN} forge="github" short="#214" />)
    const link = root.querySelector("a")!
    expect(link.getAttribute("href")).toBe(OWN)
    expect(link.getAttribute("target")).toBe("_blank")
    expect(link.textContent).toBe("#214")
    expect(link.querySelector("svg")?.getAttribute("aria-label")).toBe("GitHub")
  })

  it("draws the GitLab mark for a merge request", () => {
    const root = render(
      <ForgeLink url="https://gitlab.com/group/app/-/merge_requests/7" forge="gitlab" short="group/app#7" />,
    )
    expect(root.querySelector("svg")?.getAttribute("aria-label")).toBe("GitLab")
    expect(root.textContent).toBe("group/app#7")
  })

  it("shows the whole address when the keyboard lands on it", () => {
    const root = render(<ForgeLink url={OWN} forge="github" short="#214" />)
    act(() => root.querySelector("a")!.focus())
    expect(root.querySelector("[role=tooltip]")?.textContent).toBe(OWN)
  })
})

describe("PullRequestsMark", () => {
  const view = (repo: string, number: number, short: string) => ({
    url: `https://github.com/${repo}/pull/${number}`,
    forge: "github" as const,
    repo,
    number,
    short,
  })

  it("shows nothing on a task with no pull request", () => {
    expect(render(<PullRequestsMark pullRequests={[]} />).innerHTML).toBe("")
    expect(render(<PullRequestsMark pullRequests={undefined} />).innerHTML).toBe("")
  })

  it("counts the pull requests, and lists each one in the tooltip", () => {
    const root = render(
      <PullRequestsMark
        pullRequests={[view("acme/widgets", 214, "#214"), view("rust-lang/cargo", 1234, "rust-lang/cargo#1234")]}
      />,
    )
    const icon = root.querySelector("svg")!
    expect(icon.getAttribute("aria-label")).toBe("2 pull requests")
    act(() => {
      icon.parentElement!.dispatchEvent(new PointerEvent("pointerover", { bubbles: true }))
    })
    expect([...root.querySelectorAll("[role=tooltip] li")].map((item) => item.textContent)).toEqual([
      "#214",
      "rust-lang/cargo#1234",
    ])
  })
})
