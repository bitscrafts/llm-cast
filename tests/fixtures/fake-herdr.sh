#!/usr/bin/env sh
# spec-03 part 3 — fake herdr CLI shim for the E2E tests (R1/R10, N4).
#
# The E2E spawns the REAL mcp-server binary with a PATH whose first entry is a
# temp dir containing a `herdr` symlink resolving here, so every `herdr …`
# invocation of the driver lands on this script. It never touches a real
# socket, the live herdr server, or a real xterm (G7).
#
# Behaviour:
#   * every invocation is logged — argv joined by spaces, one line per
#     invocation — appended to $FAKE_LOG (required);
#   * a $HERDR_SOCKET_PATH containing the marker `fail` makes EVERY command
#     exit non-zero with a missing-socket error on stderr (R10: the acceptance
#     test drives this — a missing socket must surface as a tool is_error,
#     never a crash, never a JSON-RPC error);
#   * otherwise it answers the herdr CLI contract the driver uses:
#       tab list   -> an existing `agent` tab (w1:t1)
#       pane list  -> one pane (w1:p1) inside w1:t1
#       tab create / tab focus / tab close / pane run -> empty result
set -u

if [ -z "${FAKE_LOG:-}" ]; then
    echo "fake-herdr: FAKE_LOG is not set" >&2
    exit 9
fi
printf '%s\n' "$*" >>"$FAKE_LOG" || exit 9

case "${HERDR_SOCKET_PATH:-}" in
    *fail*)
        echo "herdr: connect: no such file or directory ($HERDR_SOCKET_PATH)" >&2
        exit 1
        ;;
esac

case "$1" in
    tab)
        case "${2:-}" in
            list)
                printf '%s\n' '{"id":"cli:tab:list","result":{"tabs":[{"tab_id":"w1:t1","label":"agent"}],"type":"tab_list"}}'
                ;;
            create | focus | close)
                printf '%s\n' '{"id":"cli:tab:create","result":{}}'
                ;;
            *)
                echo "fake-herdr: unexpected tab subcommand: $*" >&2
                exit 2
                ;;
        esac
        ;;
    pane)
        case "${2:-}" in
            list)
                printf '%s\n' '{"id":"cli:pane:list","result":{"panes":[{"pane_id":"w1:p1","tab_id":"w1:t1"}],"type":"pane_list"}}'
                ;;
            run)
                printf '%s\n' '{"id":"cli:pane:run","result":{}}'
                ;;
            *)
                echo "fake-herdr: unexpected pane subcommand: $*" >&2
                exit 2
                ;;
        esac
        ;;
    *)
        echo "fake-herdr: unexpected command: $*" >&2
        exit 2
        ;;
esac
exit 0
