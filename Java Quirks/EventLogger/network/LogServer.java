package EventLogger.network;

import EventLogger.util.Config;

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.net.ServerSocket;
import java.net.Socket;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

public class LogServer implements Runnable {

    private final ExecutorService threadPool = Executors.newCachedThreadPool();

    @Override
    public void run() {

        try (ServerSocket server = new ServerSocket()) {
            
            server.setReuseAddress(true);
            server.bind(new java.net.InetSocketAddress(Config.PORT));

            System.out.println(
                    "Server Listening on port " + Config.PORT + "...");

            while (true) {

                Socket client = server.accept();
                threadPool.execute(() -> handleClient(client));
            }

        } catch (Exception ex) {
            ex.printStackTrace();
        } finally {
            threadPool.shutdown();
        }
    }

    private void handleClient(Socket client) {
        try (
                BufferedReader reader = new BufferedReader(
                        new InputStreamReader(
                                client.getInputStream()))) {

            String msg = reader.readLine();

            System.out.println(
                    "Received -> "
                            + msg);

            client.close();

        } catch (Exception ex) {
            ex.printStackTrace();
        }
    }
}
