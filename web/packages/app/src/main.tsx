import { StrictMode } from "react"
import { createRoot } from "react-dom/client"
import { createBrowserRouter, RouterProvider } from "react-router-dom"

import { BOARD_ROUTE, TASK_ROUTE } from "@open-planner/task-ui"

import { App } from "./App"
import { startRealtime } from "./lib/realtime"
import { loadProjects } from "./lib/store"
import { DetailRoute } from "./routes/detail"
import { ListRoute } from "./routes/list"

import "@fontsource-variable/geist/index.css"
import "./index.css"

const router = createBrowserRouter([
  {
    element: <App />,
    children: [
      { path: "/", element: <ListRoute /> },
      { path: BOARD_ROUTE, element: <ListRoute /> },
      { path: TASK_ROUTE, element: <DetailRoute /> },
    ],
  },
])

const root = document.getElementById("root")
if (root === null) throw new Error("missing #root element")

loadProjects()
startRealtime()

createRoot(root).render(
  <StrictMode>
    <RouterProvider router={router} />
  </StrictMode>,
)
