declare module "node-roon-api" {
  class RoonApi {
    constructor(options: import("./roon-bridge.js").RoonExtensionOptions);

    init_services(services: {
      required_services: import("./roon-bridge.js").RoonServiceDescriptor[];
      provided_services: import("./roon-bridge.js").RoonServiceDescriptor[];
    }): void;
    start_discovery(): void;
    stop_discovery(): void;
    disconnect_all(): void;
    ws_connect(options: import("./roon-bridge.js").RoonConnectionOptions): {
      transport: { close(): void };
    };
  }

  export default RoonApi;
}

declare module "node-roon-api-image" {
  class RoonApiImage {
    static services: Array<{ name: string }>;
  }

  export default RoonApiImage;
}

declare module "node-roon-api-status" {
  import RoonApi from "node-roon-api";

  class RoonApiStatus {
    constructor(roon: RoonApi);

    services: Array<{ name: string }>;
    set_status(message: string, isError: boolean): void;
  }

  export default RoonApiStatus;
}

declare module "node-roon-api-transport" {
  class RoonApiTransport {
    static services: Array<{ name: string }>;
  }

  export default RoonApiTransport;
}
