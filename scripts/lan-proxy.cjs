// LAN proxy for the loopback-only `orx up` dashboard.
//
// `orx up` binds 127.0.0.1 and rejects any non-loopback Host header
// (up_remote::loopback_guard) — that guard is the CSRF boundary. This proxy
// terminates LAN connections on 0.0.0.0 and forwards them with
//   Host:   127.0.0.1:<BACKEND_PORT>
//   Origin: http://127.0.0.1:<BACKEND_PORT>   (only when the browser sent one)
// so the guard sees a loopback request. GETs pass through untouched.
//
// WARNING: `orx up` has no auth in normal mode — anyone who can reach this
// port gets the full dashboard (projects, run control, local file reads via
// the API). Expose it only on a trusted LAN.

const fs = require('node:fs')
const http = require('node:http')
const os = require('node:os')
const path = require('node:path')

const pidDir = path.join(os.homedir(), '.openresearch')
fs.mkdirSync(pidDir, { recursive: true })
const pidFile = path.join(pidDir, 'lan-proxy.pid')
fs.writeFileSync(pidFile, String(process.pid))
const removePid = () => { try { fs.unlinkSync(pidFile) } catch {} }
const stop = () => { removePid(); server.close(() => process.exit(0)) }
process.on('SIGTERM', stop)
process.on('SIGINT', stop)
process.on('exit', removePid)

const listenPort = Number(process.env.PROXY_PORT || 4792)
const targetPort = Number(process.env.BACKEND_PORT || 4791)
const targetHost = '127.0.0.1'
const loopbackAuthority = `${targetHost}:${targetPort}`

const server = http.createServer((req, res) => {
  const headers = { ...req.headers, host: loopbackAuthority }
  // Unsafe verbs with an Origin must present the loopback origin the guard
  // expects; a browser on the LAN sends its own (http://<lan-ip>:4792).
  if (headers.origin) headers.origin = `http://${loopbackAuthority}`
  const upstream = http.request(
    { host: targetHost, port: targetPort, path: req.url, method: req.method, headers },
    (upRes) => {
      res.writeHead(upRes.statusCode, upRes.headers)
      upRes.pipe(res)
    },
  )
  upstream.on('error', () => {
    res.writeHead(502)
    res.end('orx up is not reachable on ' + loopbackAuthority)
  })
  req.pipe(upstream)
})

server.listen(listenPort, '0.0.0.0', () => {
  console.log(`orx LAN proxy: http://0.0.0.0:${listenPort} -> http://${loopbackAuthority}`)
})
