#!/bin/bash

# Exit on error
set -e

# Function to display usage
usage() {
    echo "Usage: $0 {start|stop|restart|logs|clean}"
    echo "  start   - Start the URL shortener cluster"
    echo "  stop    - Stop the cluster"
    echo "  restart - Restart the cluster"
    echo "  logs    - Show logs from all nodes"
    echo "  clean   - Remove all containers, volumes, and images"
    exit 1
}

# Function to clean up everything
clean() {
    echo "Cleaning up..."
    docker-compose down -v
    docker-compose rm -f
    docker system prune -f
}

# Main script
case "$1" in
    start)
        echo "Starting URL shortener cluster..."
        docker-compose up -d
        echo "Cluster started! Access points:"
        echo "Node 1 (Leader): http://localhost:8080"
        echo "Node 2: http://localhost:8081"
        echo "Node 3: http://localhost:8082"
        ;;
    stop)
        echo "Stopping cluster..."
        docker-compose down
        ;;
    restart)
        echo "Restarting cluster..."
        docker-compose restart
        ;;
    logs)
        echo "Showing logs..."
        docker-compose logs -f
        ;;
    clean)
        clean
        ;;
    *)
        usage
        ;;
esac

read -p "Press enter to exit"

exit 0