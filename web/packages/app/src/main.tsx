import { QueryClientProvider } from "@tanstack/react-query"
import { StrictMode } from "react"
import { createRoot } from "react-dom/client"
import { createBrowserRouter, RouterProvider } from "react-router-dom"

import { BOARD_ROUTE, TAGS_ROUTE, TASK_ROUTE } from "@open-planner/task-ui"

import { App } from "./App"
import { queryClient } from "./lib/query-client"
import { startRealtime } from "./lib/realtime"
import { DetailRoute } from "./routes/detail"
import { ListRoute } from "./routes/list"
import { TagsRoute } from "./routes/tags"

import "@fontsource-variable/geist/index.css"
import "./index.css"

const router = createBrowserRouter([
  {
    element: <App />,
    children: [
      { path: "/", element: <ListRoute /> },
      { path: BOARD_ROUTE, element: <ListRoute /> },
      { path: TAGS_ROUTE, element: <TagsRoute /> },
      { path: TASK_ROUTE, element: <DetailRoute /> },
    ],
  },
])

const root = document.getElementById("root")
if (root === null) throw new Error("missing #root element")

startRealtime()

createRoot(root).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  </StrictMode>,
)
