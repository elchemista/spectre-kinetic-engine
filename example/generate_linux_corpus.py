import json

tools_old = {
    # Package Managers
    "Linux.Apt.install/1": {
        "doc": "install a package using apt package manager on debian ubuntu",
        "spec": "package: string",
        "params": [
            {"name": "package", "desc": "name of the software package", "required": "true", "type": "string", "aliases": ["name", "app", "pkg"]}
        ],
        "examples": ["INSTALL PACKAGE {package} VIA APT", "APT INSTALL {package}"],
        "al_strings": ["INSTALL PACKAGE {package} VIA APT", "APT INSTALL {package}", "INSTALL {package}"]
    },
    "Linux.Pacman.install/1": {
        "doc": "install a package using pacman package manager on arch linux",
        "spec": "package: string",
        "params": [
            {"name": "package", "desc": "name of the software package", "required": "true", "type": "string", "aliases": ["name", "app", "pkg"]}
        ],
        "examples": ["INSTALL PACKAGE {package} VIA PACMAN", "PACMAN SYNC {package}"],
        "al_strings": ["INSTALL PACKAGE {package} VIA PACMAN", "PACMAN SYNC {package}", "INSTALL {package}"]
    },
    "Linux.Dnf.install/1": {
        "doc": "install a package using dnf package manager on fedora rhel centos",
        "spec": "package: string",
        "params": [
            {"name": "package", "desc": "name of the software package", "required": "true", "type": "string", "aliases": ["name", "app", "pkg"]}
        ],
        "examples": ["INSTALL PACKAGE {package} VIA DNF", "DNF INSTALL {package}"],
        "al_strings": ["INSTALL PACKAGE {package} VIA DNF", "DNF INSTALL {package}", "INSTALL {package}"]
    },
    # Coreutils
    "Linux.Coreutils.ls/1": {
        "doc": "list directory contents files folders",
        "spec": "path: string",
        "params": [
            {"name": "path", "desc": "directory path to list", "required": "true", "type": "string", "aliases": ["dir", "folder"]}
        ],
        "examples": ["LIST DIRECTORY {path}", "LS DIRECTORY {path}"],
        "al_strings": ["LIST DIRECTORY {path}", "LS DIRECTORY {path}"]
    },
    "Linux.Coreutils.rm/1": {
        "doc": "remove delete erase file or directory",
        "spec": "path: string",
        "params": [
            {"name": "path", "desc": "file or folder path to delete", "required": "true", "type": "string", "aliases": ["file", "dir"]}
        ],
        "examples": ["DELETE FILE {path}", "REMOVE FILE {path}", "RM FILE {path}"],
        "al_strings": ["DELETE FILE {path}", "REMOVE FILE {path}", "RM FILE {path}"]
    },
    "Linux.Coreutils.cat/1": {
        "doc": "read print show concatenate file contents to standard output",
        "spec": "path: string",
        "params": [
            {"name": "path", "desc": "file path to read", "required": "true", "type": "string", "aliases": ["file"]}
        ],
        "examples": ["READ FILE {path}", "CAT FILE {path}"],
        "al_strings": ["READ FILE {path}", "CAT FILE {path}", "SHOW FILE {path}"]
    },
    "Linux.Coreutils.cp/2": {
        "doc": "copy files and directories",
        "spec": "source: string, dest: string",
        "params": [
            {"name": "source", "desc": "source file or directory to copy", "required": "true", "type": "string", "aliases": ["src", "file", "path"]},
            {"name": "dest", "desc": "destination path or directory", "required": "true", "type": "string", "aliases": ["target", "to", "dir"]}
        ],
        "examples": ["COPY FILE {source} TO {dest}", "CP {source} {dest}"],
        "al_strings": ["COPY FILE {source} TO {dest}", "CP {source} {dest}", "DUPLICATE FILE {source} INTO {dest}"]
    },
    "Linux.Coreutils.mv/2": {
        "doc": "move or rename files and directories",
        "spec": "source: string, dest: string",
        "params": [
            {"name": "source", "desc": "source file or directory to move", "required": "true", "type": "string", "aliases": ["src", "file", "path"]},
            {"name": "dest", "desc": "destination path or directory", "required": "true", "type": "string", "aliases": ["target", "to", "dir"]}
        ],
        "examples": ["MOVE FILE {source} TO {dest}", "RENAME FILE {source} TO {dest}", "MV {source} {dest}"],
        "al_strings": ["MOVE FILE {source} TO {dest}", "RENAME FILE {source} TO {dest}", "MV {source} {dest}"]
    },
    "Linux.Coreutils.mkdir/1": {
        "doc": "make directories create directory folder",
        "spec": "path: string",
        "params": [
            {"name": "path", "desc": "directory path to create", "required": "true", "type": "string", "aliases": ["dir", "folder"]}
        ],
        "examples": ["CREATE DIRECTORY {path}", "MAKE DIRECTORY {path}", "MKDIR {path}"],
        "al_strings": ["CREATE DIRECTORY {path}", "MAKE DIRECTORY {path}", "MKDIR {path}"]
    },
    "Linux.Coreutils.echo/1": {
        "doc": "display a line of text print string",
        "spec": "text: string",
        "params": [
            {"name": "text", "desc": "text to echo to output", "required": "true", "type": "string", "aliases": ["string", "msg", "message"]}
        ],
        "examples": ["ECHO TEXT {text}", "PRINT STRING {text}"],
        "al_strings": ["ECHO TEXT {text}", "PRINT STRING {text}", "SHOW MESSAGE {text}"]
    },
    "Linux.Findutils.find/2": {
        "doc": "search for files in a directory hierarchy recursively",
        "spec": "path: string, name: string",
        "params": [
            {"name": "path", "desc": "directory path to search within", "required": "true", "type": "string", "aliases": ["dir", "folder"]},
            {"name": "name", "desc": "name of the file to search for", "required": "true", "type": "string", "aliases": ["file", "pattern", "query"]}
        ],
        "examples": ["FIND FILE {name} IN DIRECTORY {path}", "SEARCH DIRECTORY {path} FOR {name}"],
        "al_strings": ["FIND FILE {name} IN DIRECTORY {path}", "SEARCH DIRECTORY {path} FOR {name}"]
    },
    "Linux.Coreutils.uname/1": {
        "doc": "print system information like kernel name operating system architecture",
        "spec": "flags: string",
        "params": [
            {"name": "flags", "desc": "flags to pass to uname like -a or -r", "required": "false", "type": "string", "aliases": ["opts", "options"]}
        ],
        "examples": ["PRINT SYSTEM INFO WITH {flags}", "UNAME WITH {flags}"],
        "al_strings": ["PRINT SYSTEM INFO WITH {flags}", "UNAME WITH {flags}", "SHOW OS INFO"]
    },
    "Linux.Procps.free/0": {
        "doc": "display amount of free and used memory in the system RAM",
        "spec": "",
        "params": [],
        "examples": ["SHOW FREE MEMORY", "DISPLAY RAM USAGE"],
        "al_strings": ["SHOW FREE MEMORY", "DISPLAY RAM USAGE", "FREE MEMORY"]
    },
    "Linux.Systemd.systemctl_start/1": {
        "doc": "start a systemd service daemon unit",
        "spec": "name: string",
        "params": [
            {"name": "name", "desc": "name of the service to start", "required": "true", "type": "string", "aliases": ["service", "unit"]}
        ],
        "examples": ["START SERVICE {name}"],
        "al_strings": ["START SERVICE {name}", "SYSTEMCTL START {name}"]
    },
    "Linux.Systemd.systemctl_stop/1": {
        "doc": "stop a systemd service daemon unit",
        "spec": "name: string",
        "params": [
            {"name": "name", "desc": "name of the service to stop", "required": "true", "type": "string", "aliases": ["service", "unit"]}
        ],
        "examples": ["STOP SERVICE {name}"],
        "al_strings": ["STOP SERVICE {name}", "SYSTEMCTL STOP {name}"]
    },
    "Linux.Archive.unzip/2": {
        "doc": "list, test and extract compressed files in a ZIP archive",
        "spec": "archive: string, dest: string",
        "params": [
            {"name": "archive", "desc": "zip archive file to extract", "required": "true", "type": "string", "aliases": ["file", "src", "zip"]},
            {"name": "dest", "desc": "directory to extract to", "required": "false", "type": "string", "aliases": ["to", "dir", "path"]}
        ],
        "examples": ["UNZIP FILE {archive} TO {dest}", "EXTRACT ZIP {archive}"],
        "al_strings": ["UNZIP FILE {archive} TO {dest}", "EXTRACT ZIP {archive}"]
    },
    "Linux.Archive.zip/2": {
        "doc": "package and compress zip archive files",
        "spec": "archive: string, files: string",
        "params": [
            {"name": "archive", "desc": "output zip filename", "required": "true", "type": "string", "aliases": ["output", "name", "dest"]},
            {"name": "files", "desc": "files or directories to zip", "required": "true", "type": "string", "aliases": ["src", "path", "inputs"]}
        ],
        "examples": ["ZIP FILES {files} INTO {archive}", "COMPRESS TO ZIP {archive}"],
        "al_strings": ["ZIP FILES {files} INTO {archive}", "COMPRESS TO ZIP {archive}"]
    }
}

tools_new = {
    # SCHEDULING & AUTOMATION
    "Linux.Cron.crontab/2": {
        "doc": "maintain crontab files for individual users",
        "spec": "schedule: string, command: string",
        "params": [
            {"name": "schedule", "desc": "cron expression for timing", "required": "true", "type": "string", "aliases": ["time", "when", "frequency"]},
            {"name": "command", "desc": "script or command to run", "required": "true", "type": "string", "aliases": ["cmd", "task", "job"]}
        ],
        "examples": ["ADD CRON JOB {command} AT {schedule}", "SET TASK FOR TODAY TO {command} AT {schedule}", "AUTOMATE {command} EVERY {schedule}"],
        "al_strings": ["ADD CRON JOB {command} AT {schedule}", "SET TASK FOR TODAY TO {command} AT {schedule}", "AUTOMATE {command} EVERY {schedule}"]
    },
    "Linux.Cron.at/2": {
        "doc": "execute commands at a later time",
        "spec": "time: string, command: string",
        "params": [
            {"name": "time", "desc": "time to execute the job", "required": "true", "type": "string", "aliases": ["when", "datetime"]},
            {"name": "command", "desc": "script or command to run", "required": "true", "type": "string", "aliases": ["cmd", "task", "job"]}
        ],
        "examples": ["SCHEDULE TASK {command} FOR {time}", "RUN ONCE COMMAND {command} AT {time}", "AT {time} DO {command}"],
        "al_strings": ["SCHEDULE TASK {command} FOR {time}", "RUN ONCE COMMAND {command} AT {time}", "AT {time} DO {command}"]
    },

    # NETWORK & DOWNLOADING
    "Linux.Network.curl/2": {
        "doc": "download transfer fetch data from network url http https web",
        "spec": "url: string, path: string",
        "params": [
            {"name": "url", "desc": "web address to download", "required": "true", "type": "string", "aliases": ["uri", "link"]},
            {"name": "path", "desc": "output file path", "required": "true", "type": "string", "aliases": ["out", "dest", "file"]}
        ],
        "examples": ["DOWNLOAD URL {url} TO FILE {path}", "CURL URL {url} TO {path}"],
        "al_strings": ["DOWNLOAD URL {url} TO FILE {path}", "CURL URL {url} TO {path}", "FETCH HTTP {url}"]
    },
    "Linux.Network.wget/2": {
        "doc": "non-interactive network downloader wget",
        "spec": "url: string, path: string",
        "params": [
            {"name": "url", "desc": "web address to download", "required": "true", "type": "string", "aliases": ["uri", "link"]},
            {"name": "path", "desc": "output file path", "required": "true", "type": "string", "aliases": ["out", "dest", "file"]}
        ],
        "examples": ["WGET URL {url} INTO {path}", "PULL WEB PAGE {url} TO {path}"],
        "al_strings": ["WGET URL {url} INTO {path}", "PULL WEB PAGE {url} TO {path}"]
    },
    "Linux.Network.aria2c/2": {
        "doc": "fast lightweight multi-protocol & multi-source download utility",
        "spec": "url: string, path: string",
        "params": [
            {"name": "url", "desc": "URL or torrent to download", "required": "true", "type": "string", "aliases": ["uri", "torrent"]},
            {"name": "path", "desc": "output file or directory", "required": "true", "type": "string", "aliases": ["out", "dest", "dir"]}
        ],
        "examples": ["ARIA DOWNLOAD {url} TO {path}", "FAST DOWNLOAD URL {url} TO {path}"],
        "al_strings": ["ARIA DOWNLOAD {url} TO {path}", "FAST DOWNLOAD URL {url} TO {path}"]
    },
    "Linux.Network.rsync/2": {
        "doc": "fast versatile remote and local file-copying tool",
        "spec": "source: string, dest: string",
        "params": [
            {"name": "source", "desc": "source file or directory", "required": "true", "type": "string", "aliases": ["src", "origin"]},
            {"name": "dest", "desc": "destination file or directory", "required": "true", "type": "string", "aliases": ["destination", "target"]}
        ],
        "examples": ["SYNC DIRECTORY {source} WITH {dest}", "RSYNC {source} TO {dest}", "BACKUP FILES FROM {source} TO {dest}"],
        "al_strings": ["SYNC DIRECTORY {source} WITH {dest}", "RSYNC {source} TO {dest}", "BACKUP FILES FROM {source} TO {dest}"]
    },
    "Linux.Network.ping/1": {
        "doc": "send ICMP ECHO_REQUEST to network hosts",
        "spec": "host: string",
        "params": [
            {"name": "host", "desc": "hostname or IP address to ping", "required": "true", "type": "string", "aliases": ["ip", "address", "server"]}
        ],
        "examples": ["PING HOST {host}", "TEST NETWORK CONNECTIVITY TO {host}", "IS HOST {host} ALIVE"],
        "al_strings": ["PING HOST {host}", "TEST NETWORK CONNECTIVITY TO {host}", "IS HOST {host} ALIVE"]
    },
    "Linux.Network.dig/1": {
        "doc": "DNS lookup utility",
        "spec": "domain: string",
        "params": [
            {"name": "domain", "desc": "domain name to query", "required": "true", "type": "string", "aliases": ["host", "name"]}
        ],
        "examples": ["LOOKUP DNS FOR {domain}", "DIG DOMAIN {domain}", "RESOLVE {domain}"],
        "al_strings": ["LOOKUP DNS FOR {domain}", "DIG DOMAIN {domain}", "RESOLVE {domain}", "NSLOOKUP {domain}"]
    },
    "Linux.Iproute2.ip/1": {
        "doc": "show / manipulate routing, network devices, interfaces and tunnels",
        "spec": "command: string",
        "params": [
            {"name": "command", "desc": "iproute2 command like addr show or link set", "required": "true", "type": "string", "aliases": ["cmd", "action", "args"]}
        ],
        "examples": ["RUN IP ROUTE COMMAND {command}", "CONFIGURE NETWORK INTERFACES", "SHOW MY IP ADDRESS"],
        "al_strings": ["RUN IP ROUTE COMMAND {command}", "CONFIGURE NETWORK INTERFACES", "SHOW MY IP ADDRESS", "IP ADDRESS SHOW"]
    },
    "Linux.Iproute2.ss/1": {
        "doc": "utility to investigate sockets",
        "spec": "flags: string",
        "params": [
            {"name": "flags", "desc": "flags like -tlnp", "required": "true", "type": "string", "aliases": ["opts", "options"]}
        ],
        "examples": ["SHOW OPEN PORTS WITH {flags}", "LIST NETWORK CONNECTIONS", "SS COMMAND WITH {flags}"],
        "al_strings": ["SHOW OPEN PORTS WITH {flags}", "LIST NETWORK CONNECTIONS", "SS COMMAND WITH {flags}", "NETSTAT CONNECTIONS"]
    },

    # SSH & REMOTE
    "Linux.OpenSSH.ssh/2": {
        "doc": "OpenSSH SSH client remote login program",
        "spec": "host: string, user: string",
        "params": [
            {"name": "host", "desc": "remote hostname or IP address", "required": "true", "type": "string", "aliases": ["ip", "server", "domain"]},
            {"name": "user", "desc": "remote username", "required": "false", "type": "string", "aliases": ["login", "username"]}
        ],
        "examples": ["SSH TO HOST {host} AS USER {user}", "CONNECT REMOTELY TO {host}", "LOGIN SECURELY TO {host}"],
        "al_strings": ["SSH TO HOST {host} AS USER {user}", "CONNECT REMOTELY TO {host}", "LOGIN SECURELY TO {host}", "SSH {user} AT {host}"]
    },
    "Linux.OpenSSH.scp/2": {
        "doc": "OpenSSH secure file copy scp",
        "spec": "source: string, dest: string",
        "params": [
            {"name": "source", "desc": "source file or directory", "required": "true", "type": "string", "aliases": ["src", "file", "path"]},
            {"name": "dest", "desc": "destination file or directory", "required": "true", "type": "string", "aliases": ["destination", "target", "to"]}
        ],
        "examples": ["SCP FILE FROM {source} TO {dest}", "SECURE COPY {source} TO {dest}", "TRANSFER FILE TO REMOTE {dest}"],
        "al_strings": ["SCP FILE FROM {source} TO {dest}", "SECURE COPY {source} TO {dest}", "TRANSFER FILE TO REMOTE {dest}"]
    },
    "Linux.OpenSSH.ssh_keygen/1": {
        "doc": "authentication key generation, management and conversion",
        "spec": "type: string",
        "params": [
            {"name": "type", "desc": "key type to generate like rsa or ed25519", "required": "true", "type": "string", "aliases": ["algo", "encryption"]}
        ],
        "examples": ["GENERATE SSH KEY OF TYPE {type}", "CREATE NEW SSH CERTIFICATE", "SSH-KEYGEN {type}"],
        "al_strings": ["GENERATE SSH KEY OF TYPE {type}", "CREATE NEW SSH CERTIFICATE", "SSH-KEYGEN {type}"]
    },

    # CONTAINERS (Docker / Podman)
    "Linux.Container.docker_run/2": {
        "doc": "run a command in a new docker container",
        "spec": "image: string, args: string",
        "params": [
            {"name": "image", "desc": "docker image to run", "required": "true", "type": "string", "aliases": ["container", "repo"]},
            {"name": "args", "desc": "arguments or command for the container", "required": "false", "type": "string", "aliases": ["cmd", "flags"]}
        ],
        "examples": ["RUN DOCKER IMAGE {image} WITH ARGS {args}", "START CONTAINER FROM {image}", "DEPLOY DOCKER APPLICATION {image}"],
        "al_strings": ["RUN DOCKER IMAGE {image} WITH ARGS {args}", "START CONTAINER FROM {image}", "DEPLOY DOCKER APPLICATION {image}"]
    },
    "Linux.Container.docker_compose/1": {
        "doc": "define and run multi-container docker applications",
        "spec": "command: string",
        "params": [
            {"name": "command", "desc": "compose command like up, down, or build", "required": "true", "type": "string", "aliases": ["cmd", "action"]}
        ],
        "examples": ["RUN DOCKER COMPOSE COMMAND {command}", "START SERVICES WITH COMPOSE {command}"],
        "al_strings": ["RUN DOCKER COMPOSE COMMAND {command}", "START SERVICES WITH COMPOSE {command}", "DOCKER-COMPOSE {command}"]
    },
    "Linux.Container.podman/2": {
        "doc": "daemonless container engine for developing, managing, and running OCI Containers",
        "spec": "command: string, args: string",
        "params": [
            {"name": "command", "desc": "podman command like run, build, ps", "required": "true", "type": "string", "aliases": ["action"]},
            {"name": "args", "desc": "arguments for podman", "required": "false", "type": "string", "aliases": ["params"]}
        ],
        "examples": ["RUN PODMAN {command} WITH {args}", "MANAGE CONTAINERS USING PODMAN {command}"],
        "al_strings": ["RUN PODMAN {command} WITH {args}", "MANAGE CONTAINERS USING PODMAN {command}"]
    },

    # GIT & DEV TOOLS
    "Linux.Vcs.git/2": {
        "doc": "the stupid content tracker git version control",
        "spec": "command: string, args: string",
        "params": [
            {"name": "command", "desc": "git command like commit, push, pull, log", "required": "true", "type": "string", "aliases": ["action"]},
            {"name": "args", "desc": "arguments for the git command", "required": "false", "type": "string", "aliases": ["params", "flags"]}
        ],
        "examples": ["RUN GIT {command} WITH ARGS {args}", "COMMIT CODE CHANGES WITH GIT {command}", "PUBLISH REPO TO GITHUB USING {command}"],
        "al_strings": ["RUN GIT {command} WITH ARGS {args}", "COMMIT CODE CHANGES WITH GIT {command}", "PUBLISH REPO TO GITHUB USING {command}"]
    },
    "Linux.Build.make/1": {
        "doc": "GNU make utility to maintain groups of programs",
        "spec": "target: string",
        "params": [
            {"name": "target", "desc": "make target like all, clean, install", "required": "false", "type": "string", "aliases": ["goal"]}
        ],
        "examples": ["BUILD PROJECT WITH MAKE {target}", "COMPILE SOURCE CODE WITH MAKE"],
        "al_strings": ["BUILD PROJECT WITH MAKE {target}", "COMPILE SOURCE CODE WITH MAKE", "RUN MAKE FOR {target}"]
    },

    # LANGUAGES & PACKAGE MANAGERS
    "Linux.Lang.python/2": {
        "doc": "an interpreted, interactive, object-oriented programming language",
        "spec": "script: string, args: string",
        "params": [
            {"name": "script", "desc": "python script file to run or -c for inline", "required": "true", "type": "string", "aliases": ["file", "code"]},
            {"name": "args", "desc": "arguments passed to python", "required": "false", "type": "string", "aliases": ["params"]}
        ],
        "examples": ["RUN PYTHON SCRIPT {script} WITH {args}", "EXECUTE PYTHON PROGRAM {script}"],
        "al_strings": ["RUN PYTHON SCRIPT {script} WITH {args}", "EXECUTE PYTHON PROGRAM {script}", "PYTHON3 {script}"]
    },
    "Linux.Lang.nodejs/2": {
        "doc": "server-side JavaScript runtime environment node",
        "spec": "script: string, args: string",
        "params": [
            {"name": "script", "desc": "javascript file to run", "required": "true", "type": "string", "aliases": ["file", "js"]},
            {"name": "args", "desc": "arguments passed to node", "required": "false", "type": "string", "aliases": ["params"]}
        ],
        "examples": ["RUN NODE SCRIPT {script} WITH {args}", "EXECUTE JAVASCRIPT FILE {script}", "NODEJS {script}"],
        "al_strings": ["RUN NODE SCRIPT {script} WITH {args}", "EXECUTE JAVASCRIPT FILE {script}", "NODEJS {script}"]
    },
    "Linux.Pkg.npm/1": {
        "doc": "node package manager",
        "spec": "command: string",
        "params": [
            {"name": "command", "desc": "npm command like install, run, build", "required": "true", "type": "string", "aliases": ["cmd", "action"]}
        ],
        "examples": ["NPM {command}", "INSTALL JS DEPENDENCIES USING NPM {command}"],
        "al_strings": ["NPM {command}", "INSTALL JS DEPENDENCIES USING NPM {command}"]
    },

    # PROCESS MONITORING
    "Linux.Procps.ps/1": {
        "doc": "report a snapshot of the current processes",
        "spec": "flags: string",
        "params": [
            {"name": "flags", "desc": "process flags like aux or -ef", "required": "true", "type": "string", "aliases": ["opts", "options"]}
        ],
        "examples": ["LIST RUNNING PROCESSES WITH {flags}", "PS {flags}", "CHECK WHAT IS RUNNING ON THE SYSTEM"],
        "al_strings": ["LIST RUNNING PROCESSES WITH {flags}", "PS {flags}", "CHECK WHAT IS RUNNING ON THE SYSTEM"]
    },
    "Linux.Procps.top/0": {
        "doc": "display Linux processes and dynamic real-time view of a running system",
        "spec": "",
        "params": [],
        "examples": ["SHOW RUNNING PROCESSES WITH TOP", "MONITOR SYSTEM TASKS", "VIEW HTOP OR TOP"],
        "al_strings": ["SHOW RUNNING PROCESSES WITH TOP", "MONITOR SYSTEM TASKS", "VIEW HTOP OR TOP"]
    },
    "Linux.Procps.kill/1": {
        "doc": "kill terminate or stop a running process by pid or send signal",
        "spec": "pid: integer",
        "params": [
            {"name": "pid", "desc": "process ID to terminate", "required": "true", "type": "integer", "aliases": ["process"]}
        ],
        "examples": ["KILL PROCESS {pid}", "TERMINATE PROCESS {pid}", "STOP APPLICATION WITH PID {pid}", "KILLALL OR PKILL {pid}"],
        "al_strings": ["KILL PROCESS {pid}", "TERMINATE PROCESS {pid}", "STOP APPLICATION WITH PID {pid}", "KILLALL OR PKILL {pid}"]
    },
    "Linux.Procps.watch/2": {
        "doc": "execute a program periodically, showing output fullscreen",
        "spec": "interval: integer, command: string",
        "params": [
            {"name": "interval", "desc": "seconds between updates", "required": "false", "type": "integer", "aliases": ["seconds"]},
            {"name": "command", "desc": "command to watch", "required": "true", "type": "string", "aliases": ["cmd"]}
        ],
        "examples": ["WATCH COMMAND {command} EVERY {interval} SECONDS", "MONITOR {command} CONSTANTLY"],
        "al_strings": ["WATCH COMMAND {command} EVERY {interval} SECONDS", "MONITOR {command} CONSTANTLY"]
    },

    # SYSTEM CONTROLS
    "Linux.Systemd.systemctl_restart/1": {
        "doc": "restart a systemd service daemon unit",
        "spec": "name: string",
        "params": [
            {"name": "name", "desc": "name of the service to restart", "required": "true", "type": "string", "aliases": ["service", "unit"]}
        ],
        "examples": ["RESTART SERVICE {name}", "SYSTEMCTL RESTART {name}", "REBOOT BACKGROUND DAEMON {name}"],
        "al_strings": ["RESTART SERVICE {name}", "SYSTEMCTL RESTART {name}", "REBOOT BACKGROUND DAEMON {name}"]
    },
    "Linux.Systemd.journalctl/1": {
        "doc": "query the systemd journal logs",
        "spec": "flags: string",
        "params": [
            {"name": "flags", "desc": "journalctl flags like -u for unit or -f for follow", "required": "false", "type": "string", "aliases": ["opts", "args"]}
        ],
        "examples": ["READ SYSTEM LOGS WITH {flags}", "CHECK JOURNALCTL FOR {flags}", "VIEW SYSTEM ERRORS"],
        "al_strings": ["READ SYSTEM LOGS WITH {flags}", "CHECK JOURNALCTL FOR {flags}", "VIEW SYSTEM ERRORS", "DMESG OR JOURNAL LOGS"]
    },

    # TEXT PROCESSING
    "Linux.Coreutils.grep/2": {
        "doc": "search text using grep find regex pattern in file",
        "spec": "pattern: string, file: string",
        "params": [
            {"name": "pattern", "desc": "regular expression or string to search for", "required": "true", "type": "string", "aliases": ["regex", "query", "text"]},
            {"name": "file", "desc": "path to the file to search in", "required": "true", "type": "string", "aliases": ["path", "dir"]}
        ],
        "examples": ["SEARCH TEXT {pattern} IN FILE {file}", "FIND STRING {pattern} INSIDE {file}", "RIPGREP OR GREP {pattern} IN {file}"],
        "al_strings": ["SEARCH TEXT {pattern} IN FILE {file}", "FIND STRING {pattern} INSIDE {file}", "RIPGREP OR GREP {pattern} IN {file}"]
    },
    "Linux.Coreutils.sed/2": {
        "doc": "stream editor for filtering and transforming text",
        "spec": "script: string, file: string",
        "params": [
            {"name": "script", "desc": "sed script commands like s/foo/bar/g", "required": "true", "type": "string", "aliases": ["command", "substitution"]},
            {"name": "file", "desc": "file to process", "required": "true", "type": "string", "aliases": ["path", "input"]}
        ],
        "examples": ["REPLACE TEXT IN {file} USING {script}", "MODIFY FILE CONTENT WITH SED {script}"],
        "al_strings": ["REPLACE TEXT IN {file} USING {script}", "MODIFY FILE CONTENT WITH SED {script}"]
    },
    "Linux.Coreutils.awk/2": {
        "doc": "pattern scanning and processing language",
        "spec": "script: string, file: string",
        "params": [
            {"name": "script", "desc": "awk program code", "required": "true", "type": "string", "aliases": ["program", "cmd"]},
            {"name": "file", "desc": "file to process", "required": "true", "type": "string", "aliases": ["path", "input"]}
        ],
        "examples": ["PROCESS COLUMNS IN {file} WITH AWK {script}", "PARSE TEXT WITH {script}", "EXTRACT FIELDS USING AWK {script}"],
        "al_strings": ["PROCESS COLUMNS IN {file} WITH AWK {script}", "PARSE TEXT WITH {script}", "EXTRACT FIELDS USING AWK {script}"]
    },
    "Linux.Coreutils.jq/2": {
        "doc": "Command-line JSON processor",
        "spec": "filter: string, file: string",
        "params": [
            {"name": "filter", "desc": "jq filter string", "required": "true", "type": "string", "aliases": ["query", "selector"]},
            {"name": "file", "desc": "JSON file to process", "required": "true", "type": "string", "aliases": ["path", "json"]}
        ],
        "examples": ["PARSE JSON {file} WITH FILTER {filter}", "EXTRACT JSON FIELDS USING JQ {filter}"],
        "al_strings": ["PARSE JSON {file} WITH FILTER {filter}", "EXTRACT JSON FIELDS USING JQ {filter}", "YQ OR JQ OVER {file}"]
    },

    # ARCHIVING
    "Linux.Archive.tar/2": {
        "doc": "compress and archive a directory into a tar gzip bzip2 xz file",
        "spec": "path: string, archive: string",
        "params": [
            {"name": "path", "desc": "directory to compress", "required": "true", "type": "string", "aliases": ["dir", "folder", "src"]},
            {"name": "archive", "desc": "output tar file name", "required": "true", "type": "string", "aliases": ["out", "dest"]}
        ],
        "examples": ["COMPRESS DIRECTORY {path} INTO FILE {archive}", "ARCHIVE EVERYTHING IN {path}", "TAR AND GZIP FOLDER {path} TO {archive}"],
        "al_strings": ["COMPRESS DIRECTORY {path} INTO FILE {archive}", "ARCHIVE EVERYTHING IN {path}", "TAR AND GZIP FOLDER {path} TO {archive}"]
    },

    # SYSTEM USERS AND PERMISSIONS
    "Linux.User.useradd/1": {
        "doc": "create a new user or update default new user information",
        "spec": "username: string",
        "params": [
            {"name": "username", "desc": "name of the new user", "required": "true", "type": "string", "aliases": ["user", "name"]}
        ],
        "examples": ["CREATE NEW USER {username}", "ADD SYSTEM USER {username}"],
        "al_strings": ["CREATE NEW USER {username}", "ADD SYSTEM USER {username}"]
    },
    "Linux.Coreutils.chmod/2": {
        "doc": "change file mode bits or permissions",
        "spec": "mode: string, path: string",
        "params": [
            {"name": "mode", "desc": "permission mode like 755 or +x", "required": "true", "type": "string", "aliases": ["perms", "permissions"]},
            {"name": "path", "desc": "file or directory path", "required": "true", "type": "string", "aliases": ["file", "dir"]}
        ],
        "examples": ["CHMOD FILE {path} TO {mode}", "CHANGE PERMISSIONS OF {path} TO {mode}", "MAKE SCRIPT EXECUTABLE {path}"],
        "al_strings": ["CHMOD FILE {path} TO {mode}", "CHANGE PERMISSIONS OF {path} TO {mode}", "MAKE SCRIPT EXECUTABLE {path}"]
    },
    "Linux.Coreutils.chown/2": {
        "doc": "change file owner and group",
        "spec": "owner: string, path: string",
        "params": [
            {"name": "owner", "desc": "user and group owner", "required": "true", "type": "string", "aliases": ["user", "group", "own"]},
            {"name": "path", "desc": "file or directory path", "required": "true", "type": "string", "aliases": ["file", "dir"]}
        ],
        "examples": ["CHOWN FILE {path} TO {owner}", "CHANGE OWNER OF {path} TO {owner}"],
        "al_strings": ["CHOWN FILE {path} TO {owner}", "CHANGE OWNER OF {path} TO {owner}"]
    },
    
    # EDITORS
    "Linux.Editor.vim/1": {
        "doc": "Vi IMproved, a programmer's text editor",
        "spec": "file: string",
        "params": [
            {"name": "file", "desc": "file to edit", "required": "true", "type": "string", "aliases": ["path", "document"]}
        ],
        "examples": ["OPEN TEXT EDITOR FOR {file}", "EDIT {file} WITH VIM NANO OR VI", "MODIFY {file} MANUALLY"],
        "al_strings": ["OPEN TEXT EDITOR FOR {file}", "EDIT {file} WITH VIM NANO OR VI", "MODIFY {file} MANUALLY"]
    },

    # HASHING
    "Linux.Coreutils.md5sum/1": {
        "doc": "compute and check MD5 message digest",
        "spec": "file: string",
        "params": [
            {"name": "file", "desc": "file to hash", "required": "true", "type": "string", "aliases": ["path", "document"]}
        ],
        "examples": ["COMPUTE HASH FOR {file}", "GET MD5SUM OR SHA256SUM FOR {file}", "CALCULATE CHECKSUM OF {file}"],
        "al_strings": ["COMPUTE HASH FOR {file}", "GET MD5SUM OR SHA256SUM FOR {file}", "CALCULATE CHECKSUM OF {file}"]
    }
}

tools = {**tools_old, **tools_new}
slots_set = set()

def emit(f, obj):
    f.write(json.dumps(obj) + "\n")

if __name__ == "__main__":
    with open("linux_corpus.jsonl", "w") as f:
        for tool_id, tool_data in tools.items():
            # Tool doc and spec
            emit(f, {"type": "tool_doc", "tool_id": tool_id, "text": tool_data["doc"]})
            emit(f, {"type": "tool_spec", "tool_id": tool_id, "text": tool_data["spec"]})
            
            # Params
            for p in tool_data["params"]:
                desc = p["desc"]
                req = "(required)" if p.get("required") == "true" else "(optional)"
                emit(f, {"type": "param_card", "tool_id": tool_id, "text": f"{p['name']}: {p['type']} {req} {desc}"})

            # Examples
            for ex in tool_data["examples"]:
                emit(f, {"type": "example", "tool_id": tool_id, "text": ex})

            # AL strings
            for al in tool_data["al_strings"]:
                emit(f, {"type": "al", "text": al})

            # Slot keys extraction
            for p in tool_data["params"]:
                slots_set.add(p["name"])

        # Create slot cards for all unique parameters discovered
        for slot in sorted(slots_set):
            emit(f, {"type": "slot_card", "text": f"{slot}={{{slot}}}"})

    print(f"Generated massive linux_corpus.jsonl with {len(tools)} tools and {len(slots_set)} unique slots.")

