import assert from "node:assert/strict";
import test from "node:test";

import type { RoonExtensionOptions } from "../src/roon-availability.js";
import { createSupportedRoonServices } from "../src/roon-services.js";

test("loads only Roon's read-only Image and extension Status services", () => {
  const noOperation = (): void => undefined;
  const options: RoonExtensionOptions = {
    extension_id: "io.roonscape.test",
    display_name: "RoonScape test",
    display_version: "0.0.0",
    publisher: "RoonScape",
    email: "test@roonscape.local",
    website: "https://github.com/gregbartell/roonscape",
    log_level: "none",
    core_paired: noOperation,
    core_unpaired: noOperation,
    get_persisted_state: () => ({}),
    set_persisted_state: noOperation,
  };

  const services = createSupportedRoonServices(options);

  assert.deepEqual(
    {
      required: services.requiredServices.flatMap((service) =>
        service.services.map(({ name }) => name),
      ),
      provided: services.status.services.map(({ name }) => name),
    },
    {
      required: ["com.roonlabs.image:1"],
      provided: ["com.roonlabs.status:1"],
    },
  );
});
