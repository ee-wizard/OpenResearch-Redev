import { createFileRoute } from "@tanstack/react-router";
import { TeamPage } from "../routePages";

export const Route = createFileRoute("/team")({ component: TeamPage });
