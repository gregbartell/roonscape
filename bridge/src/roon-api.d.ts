declare module "node-roon-api" {
  class RoonApi {
    constructor(options: import("./roon-availability.js").RoonExtensionOptions);

    init_services(services: {
      required_services: import("./roon-availability.js").RoonServiceDescriptor[];
      provided_services: import("./roon-availability.js").RoonServiceDescriptor[];
    }): void;
    start_discovery(): void;
    stop_discovery(): void;
    disconnect_all(): void;
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
