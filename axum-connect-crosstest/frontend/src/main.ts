import { ConnectError, createPromiseClient } from "@bufbuild/connect";
import { createConnectTransport } from "@bufbuild/connect-web";
import { TestService } from "@buf/grpc_grpc.bufbuild_connect-es/grpc/testing/test_connect";

const client = createPromiseClient(
  TestService,
  createConnectTransport({
    // TODO
    baseUrl: "http://localhost:3030",
    // baseUrl: config.blink.httpUri,
    // interceptors: [authInterceptor],
  })
);

async function runTests() {
  test("emptyCall", async () => {
    await client.emptyCall({});
  });

  test("unaryCall", async () => {
    await client.unaryCall({});
  });
}

runTests();

async function test(name: string, test: () => Promise<any>) {
  let a: ConnectError;
  const div = document.createElement("div");
  document.body.appendChild(div);
  try {
    await test();
    console.log(name, "passed");
    div.style.color = "green";
    div.innerText = `${name} ok`;
  } catch (e) {
    console.log(name, "failed:", e);
    div.style.color = "red";
    div.innerText = `${name} failed: ${e}`;
  }
}
