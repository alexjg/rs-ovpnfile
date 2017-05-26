
pub enum LineParseResult {
    NoMatchingCommand,
    NotEnoughArguments,
    Success(ConfigDirective),
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum ServerBridgeArg {
    NoGateway,
    GatewayConfig{gateway: String, netmask: String, pool_start_ip: String, pool_end_ip: String},
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum File {
    FilePath(String),
    InlineFileContents(String),
}

// This macro courtesy of https://stackoverflow.com/questions/44160750/how-to-generate-complex-enum-variants-with-a-macro-in-rust
macro_rules! define_config_directives {
    //Counting rules
    (@single $($_tt:tt)*) => {()};
    (@count $($tts:tt),*) => {<[()]>::len(&[$(define_config_directives!(@single $tts)),*])};

    // Start rule.
    // Note: `$(,)*` is a trick to eat any number of trailing commas.
    ( $( {$($cmd:tt)*} ),* $(,)*) => {
        // This starts the parse, giving the initial state of the output
        // (i.e. empty).  Note that the commands come after the semicolon.
        define_config_directives! { @parse {}, (args){}; $({$($cmd)*},)* }
    };

    // Termination rule: no more input.
    (
        @parse
        // $eout will be the body of the enum.
        {$($eout:tt)*},
        // $pout will be the body of the `parse_line` match.
        // We pass `args` explicitly to make sure all stages are using the
        // *same* `args` (due to identifier hygiene).
        ($args:ident){$($pout:tt)*};
        // See, nothing here?
    ) => {
        #[derive(PartialEq, Eq, Debug, Clone)]
        pub enum ConfigDirective {
            $($eout)*
                ServerBridge(ServerBridgeArg),
        }

        pub fn parse_line(command: &str, $args: &[&str]) -> LineParseResult {
            match command {
                $($pout)*
                    "server-bridge" => {
                        match $args.len() {
                            1 if $args[0] == "nogw" => LineParseResult::Success(ConfigDirective::ServerBridge(ServerBridgeArg::NoGateway)),
                            4 => LineParseResult::Success(ConfigDirective::ServerBridge(ServerBridgeArg::GatewayConfig{
                                gateway: $args[0].to_string(),
                                netmask: $args[1].to_string(),
                                pool_start_ip: $args[2].to_string(),
                                pool_end_ip: $args[3].to_string(),
                            })),
                            _ => LineParseResult::NotEnoughArguments
                        }
                    },
                _ => LineParseResult::NoMatchingCommand
            }
        }
    };

    // Rule for command with no arguments.
    (
        @parse {$($eout:tt)*}, ($pargs:ident){$($pout:tt)*};
        {
            command: $sname:expr,
            rust_name: $rname:ident,
            args: [],
            optional_args: [] $(,)*
        },
        $($tail:tt)*
    ) => {
        define_config_directives! {
            @parse
            {
                $($eout)*
                    $rname,
            },
            ($pargs){
                $($pout)*
                    $sname => LineParseResult::Success(ConfigDirective::$rname),
            };
            $($tail)*
        }
    };

    // Rule for other commands.
    (
        @parse {$($eout:tt)*}, ($pargs:ident){$($pout:tt)*};
        {
            command: $sname:expr,
            rust_name: $rname:ident,
            args: [$($args:ident),* $(,)*],
            optional_args: [$($oargs:ident),* $(,)*] $(,)*
        },
        $($tail:tt)*
    ) => {
        define_config_directives! {
            @parse
            {
                $($eout)*
                    $rname { $( $args: String, )* $( $oargs: Option<String>, )* },
            },
            ($pargs){
                $($pout)*
                    $sname => {
                        let num_required_args = define_config_directives!(@count $($args),*);
                        if $pargs.len() < num_required_args {
                            return LineParseResult::NotEnoughArguments
                        }
                        // This trickery is because macros can't count with
                        // regular integers.  We'll just use a mutable index
                        // instead.
                        let mut i = 0;
                        $(let $args = $pargs[i].into(); i += 1;)*
                            $(let $oargs = $pargs.get(i).map(|&s| s.into()); i += 1;)*
                            let _ = i; // avoid unused assignment warnings.

                        LineParseResult::Success(ConfigDirective::$rname {
                            $($args: $args,)*
                                $($oargs: $oargs,)*
                        })
                    },
            };
            $($tail)*
        }
    };
    // Rule for varargs commands.
    (
        @parse {$($eout:tt)*}, ($pargs:ident){$($pout:tt)*};
        {
            command: $sname:expr,
            rust_name: $rname:ident,
            varargs: $argname: ident
        },
        $($tail:tt)*
    ) => {
        define_config_directives! {
            @parse
            {
                $($eout)*
                    $rname { $argname: Vec<String>},
            },
            ($pargs){
                $($pout)*
                    $sname => {
                        if $pargs.len() == 0 {
                            return LineParseResult::NotEnoughArguments
                        }
                        LineParseResult::Success(ConfigDirective::$rname {
                            $argname: $pargs.iter().map(|s| s.to_string()).collect(),
                        })
                    },
            };
            $($tail)*
        }
    };
    // Rule for optional varargs commands.
    (
        @parse {$($eout:tt)*}, ($pargs:ident){$($pout:tt)*};
        {
            command: $sname:expr,
            rust_name: $rname:ident,
            optional_varargs: $argname: ident
        },
        $($tail:tt)*
    ) => {
        define_config_directives! {
            @parse
            {
                $($eout)*
                    $rname { $argname: Option<Vec<String>>},
            },
            ($pargs){
                $($pout)*
                    $sname => {
                        if $pargs.len() > 0 {
                            return LineParseResult::Success(ConfigDirective::$rname {
                                $argname: Some($pargs.iter().map(|s| s.to_string()).collect()),
                            })
                        }
                        LineParseResult::Success(ConfigDirective::$rname {
                            $argname: None,
                        })
                    },
            };
            $($tail)*
        }
    };
    // Rule for inline file commands.
    (
        @parse {$($eout:tt)*}, ($pargs:ident){$($pout:tt)*};
        {
            command: $sname:expr,
            rust_name: $rname:ident,
            inline_file: true
        },
        $($tail:tt)*
    ) => {
        define_config_directives! {
            @parse
            {
                $($eout)*
                    $rname { file: File},
            },
            ($pargs){
                $($pout)*
                    $sname => {
                        if $pargs.len() < 1 {
                            LineParseResult::NotEnoughArguments
                        } else {
                            LineParseResult::Success(ConfigDirective::$rname {
                                file: File::FilePath($pargs[0].to_string()),
                            })
                        }
                    },
            };
            $($tail)*
        }
    };
    //Rule for inline file with optional arguments
    (
        @parse {$($eout:tt)*}, ($pargs:ident){$($pout:tt)*};
        {
            command: $sname:expr,
            rust_name: $rname:ident,
            inline_file: true,
            optional_args: [$($oargs:ident),* $(,)*] $(,)*
        },
        $($tail:tt)*
    ) => {
        define_config_directives! {
            @parse
            {
                $($eout)*
                    $rname { file: File, $($oargs: Option<String>, )*},
            },
            ($pargs){
                $($pout)*
                    $sname => {
                        if $pargs.len() < 1 {
                            LineParseResult::NotEnoughArguments
                        } else {
                            let filename = File::FilePath($pargs[0].to_string());
                            let mut i = 1;
                            $(let $oargs = $pargs.get(i).map(|&s| s.into()); i += 1;)*
                                let _ = i; // avoid unused assignment warnings.

                            LineParseResult::Success(ConfigDirective::$rname {
                                file: filename,
                                $($oargs: $oargs,)*
                            })
                        }
                    },
            };
            $($tail)*
        }
    };
}

define_config_directives!{
    {command: "help", rust_name: Help, args: [], optional_args: []},
    {command: "config", rust_name: Config, args: [file], optional_args: []},
    {command: "mode", rust_name: Mode, args: [m], optional_args: []},
    {command: "local", rust_name: Local, args: [host], optional_args: []},
    {command: "remote", rust_name: Remote, args: [host], optional_args: [port, proto]},
    {command: "remote-random-hostname", rust_name: RemoteRandomHostname, args: [], optional_args: []},
    {command: "proto-force", rust_name: ProtoForce, args: [p], optional_args: []},
    {command: "remote-random", rust_name: RemoteRandom, args: [], optional_args: []},
    {command: "proto", rust_name: Proto, args: [p], optional_args: []},
    {command: "connect-retry", rust_name: ConnectRetry, args: [n], optional_args: [max]},
    {command: "connect-retry-max", rust_name: ConnectRetryMax, args: [n], optional_args: []},
    {command: "show-proxy-settings", rust_name: ShowProxySettings, args: [], optional_args: []},
    {command: "http-proxy", rust_name: HttpProxy, args: [server, port], optional_args: [authfile_or_auto_or_auto_nct, auth_method]},
    {command: "http-proxy-option", rust_name: HttpProxyOption, args: [http_proxy_option_type], optional_args: [parm]},
    {command: "http-proxy-user-type", rust_name: HttpProxyUserPass, inline_file: true},
    {command: "socks-proxy", rust_name: SocksProxy, args: [server], optional_args: [port, authfile]},
    {command: "resolv-retry", rust_name: ResolvRetry, args: [n], optional_args: []},
    {command: "float", rust_name: Float, args: [], optional_args: []},
    {command: "ipchange", rust_name: Ipchange, args: [cmd], optional_args: []},
    {command: "port", rust_name: Port, args: [port], optional_args: []},
    {command: "lport", rust_name: Lport, args: [port], optional_args: []},
    {command: "rport", rust_name: Rport, args: [port], optional_args: []},
    {command: "bind", rust_name: Bind, args: [], optional_args: [ipv6only]},
    {command: "nobind", rust_name: Nobind, args: [], optional_args: []},
    {command: "dev", rust_name: Dev, args: [devarg], optional_args: []},
    {command: "dev-type", rust_name: DevType, args: [device_type], optional_args: []},
    {command: "topology", rust_name: Topology, args: [mode], optional_args: []},
    {command: "dev-node", rust_name: DevNode, args: [node], optional_args: []},
    {command: "lladdr", rust_name: Lladdr, args: [address], optional_args: []},
    {command: "iproute", rust_name: Iproute, args: [cmd], optional_args: []},
    {command: "ifconfig", rust_name: Ifconfig, args: [l, rn], optional_args: []},
    {command: "ifconfig-noexec", rust_name: IfconfigNoexec, args: [], optional_args: []},
    {command: "ifconfig-nowarn", rust_name: IfconfigNowarn, args: [], optional_args: []},
    {command: "route", rust_name: Route, args: [network_or_ip], optional_args: [netmask, gateway, metric]},
    {command: "route-gateway", rust_name: RouteGateway, args: [gw_or_dhcp], optional_args: []},
    {command: "route-metric", rust_name: RouteMetric, args: [m], optional_args: []},
    {command: "route-delay", rust_name: RouteDelay, args: [], optional_args: [n, w]},
    {command: "route-up", rust_name: RouteUp, args: [cmd], optional_args: []},
    {command: "route-pre-down", rust_name: RoutePreDown, args: [cmd], optional_args: []},
    {command: "route-noexec", rust_name: RouteNoexec, args: [], optional_args: []},
    {command: "route-nopull", rust_name: RouteNopull, args: [], optional_args: []},
    {command: "allow-pull-fqdn", rust_name: AllowPullFqdn, args: [], optional_args: []},
    {command: "client-nat", rust_name: ClientNat, args: [snat_or_dnat, network, netmask, alias], optional_args: []},
    {command: "redirect-gateway", rust_name: RedirectGateway, varargs: flags},
    {command: "link-mtu", rust_name: LinkMtu, args: [n], optional_args: []},
    {command: "redirect-private", rust_name: RedirectPrivate, optional_varargs: flags},
    {command: "tun-mtu", rust_name: TunMtu, args: [n], optional_args: []},
    {command: "tun-mtu-extra", rust_name: TunMtuExtra, args: [n], optional_args: []},
    {command: "mtu-disc", rust_name: MtuDisc, args: [mtu_disc_type], optional_args: []},
    {command: "mtu-test", rust_name: MtuTest, args: [], optional_args: []},
    {command: "fragment", rust_name: Fragment, args: [max], optional_args: []},
    {command: "mssfix", rust_name: Mssfix, args: [max], optional_args: []},
    {command: "sndbuf", rust_name: Sndbuf, args: [size], optional_args: []},
    {command: "rcvbuf", rust_name: Rcvbuf, args: [size], optional_args: []},
    {command: "mark", rust_name: Mark, args: [value], optional_args: []},
    {command: "socket-flags", rust_name: SocketFlags, varargs: flags},
    {command: "txqueuelen", rust_name: Txqueuelen, args: [n], optional_args: []},
    {command: "shaper", rust_name: Shaper, args: [n], optional_args: []},
    {command: "inactive", rust_name: Inactive, args: [n], optional_args: [bytes]},
    {command: "ping", rust_name: Ping, args: [n], optional_args: []},
    {command: "ping-exit", rust_name: PingExit, args: [n], optional_args: []},
    {command: "ping-restart", rust_name: PingRestart, args: [n], optional_args: []},
    {command: "keepalive", rust_name: Keepalive, args: [interval, timeout], optional_args: []},
    {command: "ping-timer-rem", rust_name: PingTimerRem, args: [], optional_args: []},
    {command: "persist-tun", rust_name: PersistTun, args: [], optional_args: []},
    {command: "persist-key", rust_name: PersistKey, args: [], optional_args: []},
    {command: "persist-local-ip", rust_name: PersistLocalIp, args: [], optional_args: []},
    {command: "persist-remote-ip", rust_name: PersistRemoteIp, args: [], optional_args: []},
    {command: "mlock", rust_name: Mlock, args: [], optional_args: []},
    {command: "up", rust_name: Up, args: [cmd], optional_args: []},
    {command: "up-delay", rust_name: UpDelay, args: [], optional_args: []},
    {command: "down", rust_name: Down, args: [cmd], optional_args: []},
    {command: "down-pre", rust_name: DownPre, args: [], optional_args: []},
    {command: "up-restart", rust_name: UpRestart, args: [], optional_args: []},
    {command: "setenv", rust_name: Setenv, args: [name, value], optional_args: []},
    {command: "setenv-safe", rust_name: SetenvSafe, args: [name, value], optional_args: []},
    {command: "ignore-unknown-option", rust_name: IgnoreUnknownOption, varargs: opts},
    {command: "script-security", rust_name: ScriptSecurity, args: [level], optional_args: []},
    {command: "disable-occ", rust_name: DisableOcc, args: [], optional_args: []},
    {command: "user", rust_name: User, args: [user], optional_args: []},
    {command: "group", rust_name: Group, args: [group], optional_args: []},
    {command: "cd", rust_name: Cd, args: [dir], optional_args: []},
    {command: "chroot", rust_name: Chroot, args: [dir], optional_args: []},
    {command: "setcon", rust_name: Setcon, args: [context], optional_args: []},
    {command: "daemon", rust_name: Daemon, args: [], optional_args: [progname]},
    {command: "syslog", rust_name: Syslog, args: [], optional_args: [progname]},
    {command: "errors-to-stderr", rust_name: ErrorsToStderr, args: [], optional_args: []},
    {command: "passtos", rust_name: Passtos, args: [], optional_args: []},
    {command: "inetd", rust_name: Inetd, args: [], optional_args: [wait_or_nowait, progname]},
    {command: "log", rust_name: Log, args: [file], optional_args: []},
    {command: "log-append", rust_name: LogAppend, args: [file], optional_args: []},
    {command: "suppress-timestamps", rust_name: SuppressTimestamps, args: [], optional_args: []},
    {command: "machine-readable-output", rust_name: MachineReadableOutput, args: [], optional_args: []},
    {command: "writepid", rust_name: Writepid, args: [file], optional_args: []},
    {command: "nice", rust_name: Nice, args: [n], optional_args: []},
    {command: "fast-io", rust_name: FastIo, args: [], optional_args: []},
    {command: "multihome", rust_name: Multihome, args: [], optional_args: []},
    {command: "echo", rust_name: Echo, optional_varargs: parms},
    {command: "remap-usr1", rust_name: RemapUsr1, args: [signal], optional_args: []},
    {command: "verb", rust_name: Verb, args: [n], optional_args: []},
    {command: "status", rust_name: Status, args: [file], optional_args: [n]},
    {command: "status-version", rust_name: StatusVersion, args: [], optional_args: [n]},
    {command: "mute", rust_name: Mute, args: [n], optional_args: []},
    {command: "compress", rust_name: Compress, args: [], optional_args: [algorithm]},
    {command: "comp-lzo", rust_name: CompLzo, args: [], optional_args: [mode]},
    {command: "comp-noadapt", rust_name: CompNoadapt, args: [], optional_args: []},
    {command: "management", rust_name: Management, args: [ip, port], optional_args: [pw_file]},
    {command: "management-client", rust_name: ManagementClient, args: [], optional_args: []},
    {command: "management-query-passwords", rust_name: ManagementQueryPasswords, args: [], optional_args: []},
    {command: "management-query-proxy", rust_name: ManagementQueryProxy, args: [], optional_args: []},
    {command: "management-query-remote", rust_name: ManagementQueryRemote, args: [], optional_args: []},
    {command: "management-external-key", rust_name: ManagementExternalKey, args: [], optional_args: []},
    {command: "management-external-cert", rust_name: ManagementExternalCert, args: [certificate_hint], optional_args: []},
    {command: "management-forget-disconnect", rust_name: ManagementForgetDisconnect, args: [], optional_args: []},
    {command: "management-hold", rust_name: ManagementHold, args: [], optional_args: []},
    {command: "management-signal", rust_name: ManagementSignal, args: [], optional_args: []},
    {command: "management-log-cache", rust_name: ManagementLogCache, args: [n], optional_args: []},
    {command: "management-up-down", rust_name: ManagementUpDown, args: [], optional_args: []},
    {command: "management-client-auth", rust_name: ManagementClientAuth, args: [], optional_args: []},
    {command: "management-client-pf", rust_name: ManagementClientPf, args: [], optional_args: []},
    {command: "management-client-user", rust_name: ManagementClientUser, args: [u], optional_args: []},
    {command: "management-client-group", rust_name: ManagementClientGroup, args: [g], optional_args: []},
    {command: "plugin", rust_name: Plugin, args: [module_pathname], optional_args: [init_string]},
    {command: "keying-material-exporter", rust_name: KeyingMaterialExporter, args: [label, len], optional_args: []},
    {command: "server", rust_name: Server, args: [network, netmask], optional_args: [nopool]},
    {command: "push", rust_name: Push, args: [option], optional_args: []},
    {command: "push-reset", rust_name: PushReset, args: [], optional_args: []},
    {command: "push-remove", rust_name: PushRemove, args: [opt], optional_args: []},
    {command: "push-peer-info", rust_name: PushPeerInfo, args: [], optional_args: []},
    {command: "disable", rust_name: Disable, args: [], optional_args: []},
    {command: "ifconfig-pool", rust_name: IfconfigPool, args: [start_ip, end_ip], optional_args: [netmask]},
    {command: "ifconfig-pool-persist", rust_name: IfconfigPoolPersist, args: [file], optional_args: [seconds]},
    {command: "ifconfig-pool-linear", rust_name: IfconfigPoolLinear, args: [], optional_args: []},
    {command: "ifconfig-push", rust_name: IfconfigPush, args: [local, remote_netmask], optional_args: [alias]},
    {command: "iroute", rust_name: Iroute, args: [network], optional_args: [netmask]},
    {command: "client-to-client", rust_name: ClientToClient, args: [], optional_args: []},
    {command: "duplicate-cn", rust_name: DuplicateCn, args: [], optional_args: []},
    {command: "client-connect", rust_name: ClientConnect, args: [cmd], optional_args: []},
    {command: "client-disconnect", rust_name: ClientDisconnect, args: [cmd], optional_args: []},
    {command: "client-config-dir", rust_name: ClientConfigDir, args: [dir], optional_args: []},
    {command: "ccd-exclusive", rust_name: CcdExclusive, args: [], optional_args: []},
    {command: "tmp-dir", rust_name: TmpDir, args: [dir], optional_args: []},
    {command: "hash-size", rust_name: HashSize, args: [r, v], optional_args: []},
    {command: "bcast-buffers", rust_name: BcastBuffers, args: [n], optional_args: []},
    {command: "tcp-queue-limit", rust_name: TcpQueueLimit, args: [n], optional_args: []},
    {command: "tcp-nodelay", rust_name: TcpNodelay, args: [], optional_args: []},
    {command: "max-clients", rust_name: MaxClients, args: [n], optional_args: []},
    {command: "max-routes-per-client", rust_name: MaxRoutesPerClient, args: [n], optional_args: []},
    {command: "stale-routes-check", rust_name: StaleRoutesCheck, args: [n], optional_args: [t]},
    {command: "connect-freq", rust_name: ConnectFreq, args: [n, sec], optional_args: []},
    {command: "learn-address", rust_name: LearnAddress, args: [cmd], optional_args: []},
    {command: "auth-user-pass-verify", rust_name: AuthUserPassVerify, args: [cmd, method], optional_args: []},
    {command: "auth-gen-token", rust_name: AuthGenToken, args: [], optional_args: [lifetime]},
    {command: "opt-verify", rust_name: OptVerify, args: [], optional_args: []},
    {command: "auth-user-pass-optional", rust_name: AuthUserPassOptional, args: [], optional_args: []},
    {command: "client-cert-not-required", rust_name: ClientCertNotRequired, args: [], optional_args: []},
    {command: "verify-client-cert", rust_name: VerifyClientCert, args: [none_optional_require], optional_args: []},
    {command: "username-as-common-name", rust_name: UsernameAsCommonName, args: [], optional_args: []},
    {command: "compat-names", rust_name: CompatNames, args: [], optional_args: [no_remapping]},
    {command: "no-name-remapping", rust_name: NoNameRemapping, args: [], optional_args: []},
    {command: "port-share", rust_name: PortShare, args: [host, port], optional_args: [dir]},
    {command: "client", rust_name: Client, args: [], optional_args: []},
    {command: "pull", rust_name: Pull, args: [], optional_args: []},
    {command: "pull-filter", rust_name: PullFilter, args: [accept_or_ignore_or_reject, text], optional_args: []},
    {command: "auth-user-pass", rust_name: AuthUserPass, args: [], optional_args: [up]},
    {command: "auth-retry", rust_name: AuthRetry, args: [auth_retry_type], optional_args: []},
    {command: "static-challenge", rust_name: StaticChallenge, args: [t, e], optional_args: []},
    {command: "server-poll-timeout", rust_name: ServerPollTimeout, args: [n], optional_args: []},
    {command: "connect-timeout", rust_name: ConnectTimeout, args: [n], optional_args: []},
    {command: "explicit-exit-notify", rust_name: ExplicitExitNotify, args: [], optional_args: [n]},
    {command: "allow-recursive-routing", rust_name: AllowRecursiveRouting, args: [], optional_args: []},
    {command: "secret", rust_name: Secret, inline_file: true, optional_args: [direction]},
    {command: "key-direction", rust_name: KeyDirection, args: [direction], optional_args: []},
    {command: "auth", rust_name: Auth, args: [alg], optional_args: []},
    {command: "cipher", rust_name: Cipher, args: [alg], optional_args: []},
    {command: "ncp-ciphers", rust_name: NcpCiphers, args: [cipher_list], optional_args: []},
    {command: "ncp-disable", rust_name: NcpDisable, args: [], optional_args: []},
    {command: "keysize", rust_name: Keysize, args: [n], optional_args: []},
    {command: "prng", rust_name: Prng, args: [alg], optional_args: [nsl]},
    {command: "engine", rust_name: Engine, args: [], optional_args: [engine_name]},
    {command: "no-replay", rust_name: NoReplay, args: [], optional_args: []},
    {command: "replay-window", rust_name: ReplayWindow, args: [n], optional_args: [t]},
    {command: "mute-replay-warnings", rust_name: MuteReplayWarnings, args: [], optional_args: []},
    {command: "replay-persist", rust_name: ReplayPersist, args: [file], optional_args: []},
    {command: "no-iv", rust_name: NoIv, args: [], optional_args: []},
    {command: "use-prediction-resistance", rust_name: UsePredictionResistance, args: [], optional_args: []},
    {command: "test-crypto", rust_name: TestCrypto, args: [], optional_args: []},
    {command: "tls-auth", rust_name: TlsAuth, inline_file: true, optional_args: [direction]},
    {command: "tls-server", rust_name: TlsServer, args: [], optional_args: []},
    {command: "tls-client", rust_name: TlsClient, args: [], optional_args: []},
    {command: "ca", rust_name: Ca, inline_file: true},
    {command: "capath", rust_name: Capath, args: [dir], optional_args: []},
    {command: "dh", rust_name: Dh, inline_file: true},
    {command: "ecdh-curve", rust_name: EcdhCurve, args: [name], optional_args: []},
    {command: "cert", rust_name: Cert, inline_file: true},
    {command: "extra-certs", rust_name: ExtraCerts, inline_file: true},
    {command: "key", rust_name: Key, inline_file: true},
    {command: "tls-version-min", rust_name: TlsVersionMin, args: [version], optional_args: [or_highest]},
    {command: "tls-version-max", rust_name: TlsVersionMax, args: [version], optional_args: []},
    {command: "pkcs12", rust_name: Pkcs12, inline_file: true},
    {command: "verify-hash", rust_name: VerifyHash, args: [hash], optional_args: []},
    {command: "pkcs11-cert-private", rust_name: Pkcs11CertPrivate, varargs: providers},
    {command: "pkcs11-id", rust_name: Pkcs11Id, args: [name], optional_args: []},
    {command: "pkcs11-id-management", rust_name: Pkcs11IdManagement, args: [], optional_args: []},
    {command: "pkcs11-pin-cache", rust_name: Pkcs11PinCache, args: [seconds], optional_args: []},
    {command: "pkcs11-protected-authentication", rust_name: Pkcs11ProtectedAuthentication, varargs: providers},
    {command: "pkcs11-providers", rust_name: Pkcs11Providers, varargs: providers},
    {command: "pkcs11-private-mode", rust_name: Pkcs11PrivateMode, varargs: modes},
    {command: "cryptoapicert", rust_name: Cryptoapicert, args: [select_string], optional_args: []},
    {command: "key-method", rust_name: KeyMethod, args: [m], optional_args: []},
    {command: "tls-cipher", rust_name: TlsCipher, args: [l], optional_args: []},
    {command: "tls-timeout", rust_name: TlsTimeout, args: [n], optional_args: []},
    {command: "reneg-bytes", rust_name: RenegBytes, args: [n], optional_args: []},
    {command: "reneg-pkts", rust_name: RenegPkts, args: [n], optional_args: []},
    {command: "reneg-sec", rust_name: RenegSec, args: [n], optional_args: []},
    {command: "hand-window", rust_name: HandWindow, args: [n], optional_args: []},
    {command: "tran-window", rust_name: TranWindow, args: [n], optional_args: []},
    {command: "single-session", rust_name: SingleSession, args: [], optional_args: []},
    {command: "tls-exit", rust_name: TlsExit, args: [], optional_args: []},
    {command: "tls-crypt", rust_name: TlsCrypt, inline_file: true},
    {command: "askpass", rust_name: Askpass, args: [], optional_args: [file]},
    {command: "auth-nocache", rust_name: AuthNocache, args: [], optional_args: []},
    {command: "auth-token", rust_name: AuthToken, args: [token], optional_args: []},
    {command: "tls-verify", rust_name: TlsVerify, args: [cmd], optional_args: []},
    {command: "tls-export-cert", rust_name: TlsExportCert, args: [directory], optional_args: []},
    {command: "x509-username-field", rust_name: X509UsernameField, args: [fieldname], optional_args: []},
    {command: "verify-x509-name", rust_name: VerifyX509Name, args: [name, verify_x509_name_type], optional_args: []},
    {command: "x509-track", rust_name: X509Track, args: [attribute], optional_args: []},
    {command: "ns-cert-type", rust_name: NsCertType, args: [client_or_server], optional_args: []},
    {command: "remote-cert-ku", rust_name: RemoteCertKu, varargs: values},
    {command: "remote-cert-eku", rust_name: RemoteCertEku, args: [oid], optional_args: []},
    {command: "remote-cert-tls", rust_name: RemoteCertTls, args: [client_or_server], optional_args: []},
    {command: "crl-verify", rust_name: CrlVerify, inline_file: true, optional_args: [direction]},
    {command: "show-ciphers", rust_name: ShowCiphers, args: [], optional_args: []},
    {command: "show-digests", rust_name: ShowDigests, args: [], optional_args: []},
    {command: "show-tls", rust_name: ShowTls, args: [], optional_args: []},
    {command: "show-engines", rust_name: ShowEngines, args: [], optional_args: []},
    {command: "show-curves", rust_name: ShowCurves, args: [], optional_args: []},
    {command: "genkey", rust_name: Genkey, args: [], optional_args: []},
    {command: "mktun", rust_name: Mktun, args: [], optional_args: []},
    {command: "rmtun", rust_name: Rmtun, args: [], optional_args: []},
    {command: "win-sys", rust_name: WinSys, args: [path], optional_args: []},
    {command: "ip-win32", rust_name: IpWin32, args: [method], optional_args: []},
    {command: "route-method", rust_name: RouteMethod, args: [m], optional_args: []},
    {command: "dhcp-option", rust_name: DhcpOption, args: [dhcp_option_type], optional_args: [parm]},
    {command: "tap-sleep", rust_name: TapSleep, args: [n], optional_args: []},
    {command: "show-net-up", rust_name: ShowNetUp, args: [], optional_args: []},
    {command: "block-outside-dns", rust_name: BlockOutsideDns, args: [], optional_args: []},
    {command: "dhcp-renew", rust_name: DhcpRenew, args: [], optional_args: []},
    {command: "dhcp-release", rust_name: DhcpRelease, args: [], optional_args: []},
    {command: "register-dns", rust_name: RegisterDns, args: [], optional_args: []},
    {command: "pause-exit", rust_name: PauseExit, args: [], optional_args: []},
    {command: "service", rust_name: Service, args: [exit_event], optional_args: [initial_state_of_event]},
    {command: "show-adapters", rust_name: ShowAdapters, args: [], optional_args: []},
    {command: "allow-nonadmin", rust_name: AllowNonadmin, args: [], optional_args: [tap_adapter]},
    {command: "show-valid-subnets", rust_name: ShowValidSubnets, args: [], optional_args: []},
    {command: "show-net", rust_name: ShowNet, args: [], optional_args: []},
    {command: "show-pkcs11-ids", rust_name: ShowPkcs11Ids, args: [], optional_args: [provider, cert_private]},
    {command: "show-gateway", rust_name: ShowGateway, args: [], optional_args: [v6target]},
    {command: "ifconfig-ipv6", rust_name: IfconfigIpv6, args: [ipv6addr, ipv6remote], optional_args: []},
    {command: "route-ipv6", rust_name: RouteIpv6, args: [ipv6addr], optional_args: [gateway, metric]},
    {command: "server-ipv6", rust_name: ServerIpv6, args: [ipv6addr], optional_args: []},
    {command: "ifconfig-ipv6-pool", rust_name: IfconfigIpv6Pool, args: [ipv6addr], optional_args: []},
    {command: "ifconfig-ipv6-push", rust_name: IfconfigIpv6Push, args: [ipv6addr, ipv6remote], optional_args: []},
    {command: "iroute-ipv6", rust_name: IrouteIpv6, args: [ipv6addr], optional_args: []},
}
