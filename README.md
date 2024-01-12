# DNX

DNX is a DNS forwarding server, ideal for environments with complex network configurations, such as those involving multiple domains or NAT (Network Address Translation). Unlike traditional DNS servers that store and manage DNS records, DNX specializes in intelligently forwarding DNS queries to the appropriate upstream servers based on the domain being resolved. It offers the added functionality of modifying DNS responses to accommodate NAT scenarios, making it highly suitable for managed service providers or networks with VPNs and NAT setups.

## Features

- **DNS Forwarding**: Efficiently forwards DNS queries to configured upstream servers based on the domain name in the query.
- **NAT Support**: Capable of modifying DNS responses to work seamlessly in NAT environments.
- **Flexible Operation**: Can be run as a standalone application or installed as a Windows service.
- **Simple Configuration**: Uses a JSON configuration file to define DNS zones, upstream servers, and NAT rules.

## Getting Started

### Running as a Standalone Application

1. Build the application:
   ```shell
   cargo build --bin dnx --release
   ```
2. Run the executable:
   ```shell
   ./target/release/dnx.exe
   ```
3. To exit, use `Ctrl+C`.

### Installing as a Windows Service

1. Build the service and the installer:
   ```shell
   cargo build --bin service --release
   cargo build --bin service-installer --release
   ```
2. Run the installer as an administrator:
   ```shell
   ./target/release/service-installer.exe
   ```
3. Start the service manually from the Windows Services Manager, or restart your computer.

## Configuration

Configure DNX through the `%ProgramData%\dnx\dnx.json` JSON file. This configuration defines DNS forwarding zones, upstream DNS servers, and optional NAT (Network Address Translation) settings. The structure of the configuration file is outlined below:

```json
{
  "zones": [
    {
      "zone": "example.com.",
      "server": "192.168.0.1",
      "nat": {
        "ip_original": "192.168.0.0",
        "ip_translation": "10.0.0.0",
        "mask": "255.255.0.0"
      }
    },
    {
      "zone": "another-example.org.",
      "server": "192.168.1.1"
    }
  ],
  "tcp_port": 53,
  "udp_port": 53,
  "default_server": "1.1.1.1"
}
```

### Configuration Fields

- `zones`: A collection of DNS zones along with their corresponding upstream server configurations.
  - `zone`: Specifies the suffix for DNS request matching. The zone name must end with a period, such as "example.com.".
  - `server`: Defines the IP address of the designated upstream DNS server for the zone.
  - `nat` (Optional): Configures NAT for modifying DNS responses.
    - `ip_original`: Sets the host IP range used alongside the mask to determine if responses should undergo NAT.
    - `ip_translation`: Specifies the translated IP range for NAT-ed responses.
    - `mask`: Establishes the network mask for applying NAT rules.
- `tcp_port` & `udp_port`: Designates the TCP and UDP ports on which the server will listen for DNS queries.
- `default_server`: Sets a default upstream DNS server IP to be used for DNS requests that don't match any of the specified zones.

## Contributing

We warmly welcome contributions to DNX! If you have an idea for an improvement or have found a bug, here’s how you can contribute:

- **Submitting Issues**: Before creating a pull request, consider opening an issue to discuss the proposed changes or the bug you’ve identified. While it’s not mandatory to submit an issue before a pull request, doing so can significantly improve the chances of your pull request being accepted. An issue provides a clear context and justification for the changes, facilitating a smoother review process.

- **Pull Requests**: If your changes address an existing issue, please reference it in your pull request. This helps us understand the background and evaluate the contribution more effectively.

- **Code Contributions**: When submitting a pull request, ensure your code follows the existing code style and includes any necessary documentation updates.

Your contributions, whether they're suggestions, code, or feedback, are invaluable in making DNX a robust and versatile tool. Let's collaborate to enhance its capabilities!
